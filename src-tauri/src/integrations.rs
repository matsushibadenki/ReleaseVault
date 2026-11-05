use crate::{
    model::{key_ok, Binding, Document, Scope},
    vault::{self, Key, R},
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use std::{
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
};
use zeroize::Zeroizing;
pub const PRIVATE: &str = "TAURI_SIGNING_PRIVATE_KEY";
pub const PUBLIC: &str = "TAURI_UPDATER_PUBLIC_KEY";
pub const PASS: &str = "TAURI_SIGNING_PRIVATE_KEY_PASSWORD";
pub fn generate(d: &mut Document, s: &Scope, key: &Key) -> R<()> {
    for k in [PRIVATE, PUBLIC, PASS] {
        d.def(s, k)?;
        if d.entry(s, k).is_ok() {
            return Err("occupied".into());
        }
    }
    let pass = Zeroizing::new(B64.encode(vault::random::<32>()));
    let mut pk = vec![];
    let mut sk = Zeroizing::new(vec![]);
    minisign::KeyPair::generate_and_write_encrypted_keypair(
        &mut pk,
        &mut *sk,
        None,
        Some(pass.to_string()),
    )
    .map_err(|_| "crypto")?;
    for (k, v) in [
        (PRIVATE, Zeroizing::new(B64.encode(&sk))),
        (PUBLIC, Zeroizing::new(B64.encode(pk))),
        (PASS, pass),
    ] {
        d.save(s, k, v.as_bytes(), "", key)?
    }
    d.event(s, PUBLIC, "generate");
    Ok(())
}
pub fn value(d: &Document, s: &Scope, k: &str, key: &Key) -> R<Zeroizing<Vec<u8>>> {
    vault::unseal(key, &d.entry(s, k)?.cipher)
}
pub fn check_conf(d: &mut Document, s: &Scope, b: &[u8], key: &Key) -> R<()> {
    let j: serde_json::Value = serde_json::from_slice(b).map_err(|_| "format")?;
    let cfg = j
        .pointer("/plugins/updater/pubkey")
        .and_then(|v| v.as_str())
        .ok_or("mismatch")?;
    if cfg.trim().as_bytes() != value(d, s, PUBLIC, key)?.trim_ascii() {
        return Err("mismatch".into());
    }
    let raw = B64.decode(cfg.trim()).map_err(|_| "format")?;
    minisign_verify::PublicKey::decode(std::str::from_utf8(&raw).map_err(|_| "format")?)
        .map_err(|_| "format")?;
    let endpoints = j
        .pointer("/plugins/updater/endpoints")
        .and_then(|v| v.as_array())
        .ok_or("mismatch")?;
    let endpoint = value(d, s, "TAURI_UPDATER_ENDPOINT", key)?;
    if !endpoints
        .iter()
        .filter_map(|v| v.as_str())
        .any(|v| v.starts_with("https://") && v.as_bytes() == endpoint.trim_ascii())
    {
        return Err("mismatch".into());
    }
    d.event(s, PUBLIC, "config-match");
    Ok(())
}
pub fn verify_artifact(d: &Document, s: &Scope, path: &Path, sig: &[u8], key: &Key) -> R<()> {
    let pk = B64
        .decode(value(d, s, PUBLIC, key)?.trim_ascii())
        .map_err(|_| "format")?;
    let sig = B64.decode(sig.trim_ascii()).map_err(|_| "format")?;
    let pk = minisign_verify::PublicKey::decode(std::str::from_utf8(&pk).map_err(|_| "format")?)
        .map_err(|_| "format")?;
    let sig = minisign_verify::Signature::decode(std::str::from_utf8(&sig).map_err(|_| "format")?)
        .map_err(|_| "format")?;
    let mut verifier = pk.verify_stream(&sig).map_err(|_| "mismatch")?;
    let mut f = std::fs::File::open(path).map_err(|_| "file")?;
    let mut buf = [0; 65536];
    loop {
        let n = f.read(&mut buf).map_err(|_| "file")?;
        if n == 0 {
            break;
        }
        verifier.update(&buf[..n])
    }
    verifier.finalize().map_err(|_| "mismatch".into())
}
pub fn dest(repo: &str, env: &str) -> R<String> {
    let ok = |v: &str| {
        !v.is_empty()
            && v.len() <= 100
            && v.as_bytes()[0].is_ascii_alphanumeric()
            && v.bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
    };
    let mut p = repo.split('/');
    if !ok(p.next().unwrap_or(""))
        || !ok(p.next().unwrap_or(""))
        || p.next().is_some()
        || (!env.is_empty() && !ok(env))
    {
        return Err("destination".into());
    }
    Ok(format!("github.com/{repo}"))
}
fn gh() -> Command {
    #[cfg(target_os = "macos")]
    let path = ["/opt/homebrew/bin/gh", "/usr/local/bin/gh"]
        .into_iter()
        .find(|p| Path::new(p).is_file())
        .unwrap_or("gh");
    #[cfg(not(target_os = "macos"))]
    let path = "gh";
    let mut c = Command::new(path);
    c.env("GH_HOST", "github.com")
        .env("GH_PROMPT_DISABLED", "1")
        .env("GH_NO_UPDATE_NOTIFIER", "1")
        .env_remove("GH_DEBUG")
        .env_remove("DEBUG");
    c
}
pub fn github(d: &mut Document, s: &Scope, k: &str, repo: &str, env: &str, key: &Key) -> R<()> {
    let target = dest(repo, env)?;
    if !key_ok(k) {
        return Err("reserved".into());
    }
    let is_secret = d.def(s, k)?.kind == "secret";
    let bytes = value(d, s, k, key)?;
    if bytes.len() > 48 * 1024 {
        return Err("limit".into());
    }
    let mut cmd = gh();
    cmd.args([
        if is_secret { "secret" } else { "variable" },
        "set",
        k,
        "--repo",
        &target,
    ]);
    if !env.is_empty() {
        cmd.args(["--env", env]);
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "gh")?;
    let written = child.stdin.take().ok_or("gh")?.write_all(&bytes);
    let ok = wait(&mut child)?;
    if !ok || written.is_err() {
        return Err("gh".into());
    }
    d.entries.get_mut(&Document::address(s, k)).unwrap().synced = format!(
        "{} · {}{}{}",
        chrono::Utc::now().format("%Y-%m-%d %H:%M UTC"),
        target,
        if env.is_empty() { "" } else { " / " },
        env
    );
    d.entries.get_mut(&Document::address(s, k)).unwrap().binding = Some(Binding {
        repo: repo.into(),
        environment: env.into(),
        ..Default::default()
    });
    d.event(s, k, "github");
    Ok(())
}
pub fn certificate(bytes: &[u8]) -> R<(String, String)> {
    use x509_parser::prelude::*;
    let der = if bytes.starts_with(b"-----BEGIN") {
        parse_x509_pem(bytes).map_err(|_| "format")?.1.contents
    } else {
        bytes.to_vec()
    };
    let (_, cert) = parse_x509_certificate(&der).map_err(|_| "format")?;
    let expiry = chrono::DateTime::from_timestamp(cert.validity().not_after.timestamp(), 0)
        .ok_or("date")?
        .format("%Y-%m-%d")
        .to_string();
    Ok((
        expiry,
        format!("{} · #{}", cert.subject(), cert.raw_serial_as_string()),
    ))
}
pub fn csv_import(d: &mut Document, s: &Scope, b: &[u8], key: &Key) -> R<usize> {
    let mut r = csv::Reader::from_reader(b);
    let h = r.headers().map_err(|_| "format")?.clone();
    if h.iter().collect::<Vec<_>>() != ["key", "name", "kind", "value", "expires"] {
        return Err("csv".into());
    }
    let mut seen = std::collections::HashSet::new();
    let mut n = 0;
    for record in r.records() {
        let row = record.map_err(|_| "csv")?;
        let k = &row[0];
        if !seen.insert(k.to_owned()) || d.entry(s, k).is_ok() {
            return Err("occupied".into());
        }
        if d.def(s, k).is_err() {
            d.custom(s, k, &row[1], &row[2])?
        } else if d.def(s, k)?.kind != row[2] {
            return Err("invalid".into());
        }
        if !row[3].is_empty() {
            d.save(s, k, row[3].as_bytes(), &row[4], key)?
        }
        n += 1;
    }
    if n == 0 {
        return Err("csv".into());
    }
    d.event(s, "", "csv-import");
    Ok(n)
}
pub fn csv_export(d: &Document, s: &Scope) -> R<Vec<u8>> {
    let mut w = csv::Writer::from_writer(vec![]);
    w.write_record(["key", "name", "kind", "value", "expires"])
        .map_err(|_| "csv")?;
    for def in d.defs(s)? {
        let safe = if def.name.trim_start().starts_with(['=', '+', '-', '@'])
            || def.name.starts_with(['\t', '\r', '\n'])
        {
            format!("'{}", def.name)
        } else {
            def.name.clone()
        };
        w.write_record([
            &def.key,
            &safe,
            &def.kind,
            "",
            d.entry(s, &def.key)
                .map(|e| e.expires.as_str())
                .unwrap_or(""),
        ])
        .map_err(|_| "csv")?
    }
    w.into_inner().map_err(|_| "csv".into())
}
fn wait(c: &mut std::process::Child) -> R<bool> {
    let start = std::time::Instant::now();
    loop {
        if let Some(st) = c.try_wait().map_err(|_| "gh")? {
            return Ok(st.success());
        }
        if start.elapsed() > std::time::Duration::from_secs(25) {
            let _ = c.kill();
            let _ = c.wait();
            return Err("gh".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(50))
    }
}
pub fn fetch_remote(d: &mut Document, s: &Scope, k: &str) -> R<()> {
    let binding = d.entry(s, k)?.binding.as_ref().ok_or("missing")?;
    let repo = dest(&binding.repo, &binding.environment)?;
    if !key_ok(k) {
        return Err("reserved".into());
    }
    let mut c = gh();
    let jq = format!(".[] | select(.name == \"{k}\") | .updatedAt");
    c.args([
        if d.def(s, k)?.kind == "secret" {
            "secret"
        } else {
            "variable"
        },
        "list",
        "--repo",
        &repo,
        "--json",
        "name,updatedAt",
        "--jq",
        &jq,
    ]);
    if !binding.environment.is_empty() {
        c.args(["--env", &binding.environment]);
    }
    let mut child = c
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "gh")?;
    if !wait(&mut child)? {
        return Err("gh".into());
    }
    let mut text = String::new();
    child
        .stdout
        .take()
        .ok_or("gh")?
        .take(128)
        .read_to_string(&mut text)
        .map_err(|_| "gh")?;
    let remote = text.trim();
    if !remote.is_empty() {
        chrono::DateTime::parse_from_rfc3339(remote).map_err(|_| "gh")?;
    }
    let b = d
        .entries
        .get_mut(&Document::address(s, k))
        .and_then(|e| e.binding.as_mut())
        .ok_or("missing")?;
    b.remote = remote.into();
    b.polled = chrono::Utc::now().to_rfc3339();
    Ok(())
}
