use super::*;
use crate::integrations::*;
use std::process::{Command, Stdio};
use tempfile::tempdir;
const P: &str = "test-only-passphrase-590";
#[test]
fn vault_restart_authentication_and_portable_recovery() {
    let tmp = tempdir().unwrap();
    let path = tmp.path().join("vault.db");
    let mut v = Vault::open(&path).unwrap();
    let s = Scope::default();
    let sentinel = b"test_secret NOT on disk 7b81cd";
    assert!(v.create("short").is_err());
    v.create(P).unwrap();
    v.change(|d, k| d.save(&s, "APPLE_CERTIFICATE_PASSWORD", sentinel, "2030-01-01", k))
        .unwrap();
    let data = v.backup(P).unwrap();
    drop(v);
    for b in [data.as_slice(), &std::fs::read(&path).unwrap()] {
        assert!(!b.windows(sentinel.len()).any(|w| w == sentinel))
    }
    let mut v = Vault::open(&path).unwrap();
    assert!(v.unlocked().is_err());
    assert!(v.unlock("incorrect").is_err());
    v.unlock(P).unwrap();
    let sk = v.unlocked().unwrap();
    assert_eq!(
        &**value(&sk.doc, &s, "APPLE_CERTIFICATE_PASSWORD", &sk.key).unwrap(),
        sentinel
    );
    let mut second = Vault::open(&tmp.path().join("recover")).unwrap();
    assert!(second.restore(&data, "incorrect").is_err());
    assert!(!second.exists().unwrap());
    second.restore(&data, P).unwrap();
    assert!(second.restore(&data, P).is_err());
    let mut broken: serde_json::Value = serde_json::from_slice(&data).unwrap();
    let mut cipher = STANDARD.decode(broken["body"].as_str().unwrap()).unwrap();
    cipher[35] ^= 1;
    broken["body"] = json!(STANDARD.encode(cipher));
    let mut other = Vault::open(&tmp.path().join("broken")).unwrap();
    assert!(other
        .restore(&serde_json::to_vec(&broken).unwrap(), P)
        .is_err());
    assert!(!other.exists().unwrap());
}
#[test]
fn profiles_share_only_intended_credentials_and_isolate_environments() {
    let key = Zeroizing::new(vault::random());
    let mut d = Document::initial();
    let direct = Scope::default();
    let store = Scope {
        target: "mac-store".into(),
        ..direct.clone()
    };
    d.save(&direct, "APPLE_TEAM_ID", b"TEAM", "", &key).unwrap();
    assert!(d.entry(&store, "APPLE_TEAM_ID").is_ok());
    assert!(d.def(&store, PRIVATE).is_err());
    assert!(d.def(&direct, "APPLE_APP_SANDBOX").is_err());
    assert!(d
        .entry(
            &Scope {
                env: "Staging".into(),
                ..store.clone()
            },
            "APPLE_TEAM_ID"
        )
        .is_err());
    let id = d.add_project("Test", vec!["mac-store".into()]).unwrap();
    assert!(d
        .entry(
            &Scope {
                project: id,
                ..store
            },
            "APPLE_TEAM_ID"
        )
        .is_err());
}
#[test]
fn csv_is_atomic_and_exports_no_values() {
    let tmp = tempdir().unwrap();
    let mut v = Vault::open(&tmp.path().join("csv")).unwrap();
    v.create(P).unwrap();
    let s = Scope::default();
    let invalid=b"key,name,kind,value,expires\nCUSTOM_KEY,First,secret,superprivate,\nCUSTOM_KEY,Duplicate,secret,no,\n";
    assert!(v.change(|d, k| csv_import(d, &s, invalid, k)).is_err());
    assert!(v.unlocked().unwrap().doc.def(&s, "CUSTOM_KEY").is_err());
    let good=b"key,name,kind,value,expires\r\nCUSTOM_KEY,\"=SUM(1,2)\",secret,\"test,content\nmultiline\",2031-05-01\r\n";
    assert_eq!(v.change(|d, k| csv_import(d, &s, good, k)).unwrap(), 1);
    let exported = String::from_utf8(csv_export(&v.unlocked().unwrap().doc, &s).unwrap()).unwrap();
    assert!(exported.contains("'=SUM"));
    assert!(!exported.contains("multiline"));
}
#[test]
fn rejects_unexpected_github_destinations_and_reserved_names() {
    for r in [
        "-R/evil",
        "foo/bar/extra",
        "github.example:owner/repo",
        "../repo",
        "good/repo?x",
        "owner/",
    ] {
        assert!(dest(r, "").is_err())
    }
    assert!(dest("owner/repo", "--help").is_err());
    assert!(dest("org/release-vault", "Production").is_ok());
    assert!(!model::key_ok("GITHUB_TOKEN"));
    assert!(!model::key_ok("-R"));
}
#[test]
fn tauri_cli_signatures_verify_and_tampering_fails() {
    let key = Zeroizing::new(vault::random());
    let mut d = Document::initial();
    let s = Scope::default();
    generate(&mut d, &s, &key).unwrap();
    assert!(generate(&mut d, &s, &key).is_err());
    let tmp = tempdir().unwrap();
    let artifact = tmp.path().join("sample.bin");
    std::fs::write(&artifact, b"ReleaseVault test artifact").unwrap();
    let private = value(&d, &s, PRIVATE, &key).unwrap();
    let pass = value(&d, &s, PASS, &key).unwrap();
    let cli =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../node_modules/@tauri-apps/cli/tauri.js");
    let result = Command::new("node")
        .arg(cli)
        .args(["signer", "sign", "--app-version", "1.0.0"])
        .arg(&artifact)
        .env(PRIVATE, std::str::from_utf8(&private).unwrap())
        .env(PASS, std::str::from_utf8(&pass).unwrap())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(
        result.success(),
        "the installed official Tauri CLI must be able to sign"
    );
    let sig = std::fs::read(tmp.path().join("sample.bin.sig")).unwrap();
    verify_artifact(&d, &s, &artifact, &sig, &key).unwrap();
    let manifest = publishing::manifest(
        &d,
        &s,
        &key,
        &artifact,
        &sig,
        publishing::ReleaseInfo {
            version: "1.0.0",
            platform: "darwin-aarch64",
            url: "https://example.test/sample.bin",
        },
    )
    .unwrap();
    d.projects[0]
        .targets
        .extend(["windows-direct".into(), "linux-appimage".into()]);
    for (target, platform) in [
        ("mac-direct", "darwin-aarch64"),
        ("windows-direct", "windows-x86_64"),
        ("linux-appimage", "linux-x86_64"),
    ] {
        let scope = Scope {
            target: target.into(),
            ..s.clone()
        };
        publishing::add_platform(
            &mut d,
            &scope,
            &key,
            &artifact,
            &sig,
            publishing::ReleaseInfo {
                version: "1.0.0",
                platform,
                url: "https://example.test/sample.bin",
            },
        )
        .unwrap();
    }
    let entry = d.release_drafts[&publishing::ReleaseDraft::address(&s)].platforms
        ["darwin-aarch64"]
        .clone();
    remote_artifact::tests::signed_https_cases(&entry, &std::fs::read(&artifact).unwrap());
    let check = publishing::RemoteCheck {
        checked: "2026-11-06T00:00:00Z".into(),
        status: "verified".into(),
        bytes: Some(26),
    };
    publishing::record_remote_check(
        &mut d,
        &s,
        &key,
        "darwin-aarch64",
        "1.0.0",
        &entry,
        check.clone(),
    )
    .unwrap();
    let mut stale = entry.clone();
    stale.url = "https://example.test/old".into();
    assert!(publishing::record_remote_check(
        &mut d,
        &s,
        &key,
        "darwin-aarch64",
        "1.0.0",
        &stale,
        check.clone()
    )
    .is_err());
    assert!(publishing::record_remote_check(
        &mut d,
        &s,
        &key,
        "darwin-aarch64",
        "0.9.0",
        &entry,
        check.clone()
    )
    .is_err());
    let original = serde_json::to_vec(&d.release_drafts).unwrap();
    assert!(publishing::add_platform(
        &mut d,
        &s,
        &key,
        &artifact,
        &sig,
        publishing::ReleaseInfo {
            version: "1.0.1",
            platform: "darwin-x86_64",
            url: "https://example.test/sample.bin"
        }
    )
    .is_err());
    assert!(publishing::add_platform(
        &mut d,
        &s,
        &key,
        &artifact,
        &sig,
        publishing::ReleaseInfo {
            version: "1.0.0",
            platform: "darwin-aarch64",
            url: "https://example.test/sample.bin"
        }
    )
    .is_err());
    assert_eq!(original, serde_json::to_vec(&d.release_drafts).unwrap());
    let restored: Document = serde_json::from_slice(&serde_json::to_vec(&d).unwrap()).unwrap();
    let combined: Value =
        serde_json::from_slice(&publishing::combined_manifest(&restored, &s, &key).unwrap())
            .unwrap();
    assert_eq!(combined["platforms"].as_object().unwrap().len(), 3);
    assert_eq!(
        combined["platforms"]["windows-x86_64"]["signature"],
        std::str::from_utf8(sig.trim_ascii()).unwrap()
    );
    assert!(publishing::combined_manifest(
        &d,
        &Scope {
            env: "Staging".into(),
            ..s.clone()
        },
        &key
    )
    .is_err());
    let mut changed = restored;
    changed
        .save(&s, PUBLIC, b"changed key", "", &key)
        .unwrap_err();
    // Force a stale verification record as if a previously configured key had been replaced.
    changed
        .entries
        .get_mut(&Document::address(&s, PUBLIC))
        .unwrap()
        .cipher = vault::seal(&key, b"another-key").unwrap();
    assert!(publishing::combined_manifest(&changed, &s, &key).is_err());
    publishing::remove_platform(&mut d, &s, "linux-x86_64").unwrap();
    assert_eq!(
        d.release_drafts[&publishing::ReleaseDraft::address(&s)]
            .platforms
            .len(),
        2
    );
    let m: Value = serde_json::from_slice(&manifest).unwrap();
    assert_eq!(
        m["platforms"]["darwin-aarch64"]["url"],
        "https://example.test/sample.bin"
    );
    assert!(publishing::manifest(
        &d,
        &s,
        &key,
        &artifact,
        &sig,
        publishing::ReleaseInfo {
            version: "1.0.0",
            platform: "windows-x86_64",
            url: "https://example.test/sample.bin"
        },
    )
    .is_err());
    assert!(publishing::manifest(
        &d,
        &s,
        &key,
        &artifact,
        &sig,
        publishing::ReleaseInfo {
            version: "1.0.1",
            platform: "darwin-aarch64",
            url: "https://example.test/sample.bin"
        }
    )
    .is_err());
    std::fs::write(&artifact, b"changed").unwrap();
    assert!(verify_artifact(&d, &s, &artifact, &sig, &key).is_err());
    let pk = value(&d, &s, PUBLIC, &key).unwrap();
    let conf = json!({"plugins":{"updater":{"pubkey":std::str::from_utf8(&pk).unwrap(),"endpoints":["https://example.test/latest.json"]}}});
    d.save(
        &s,
        "TAURI_UPDATER_ENDPOINT",
        b"https://example.test/latest.json",
        "",
        &key,
    )
    .unwrap();
    check_conf(&mut d, &s, &serde_json::to_vec(&conf).unwrap(), &key).unwrap();
    assert!(check_conf(&mut d, &s, b"{}", &key).is_err());
}

#[cfg(unix)]
#[test]
fn github_connector_uses_stdin_scope_and_only_records_success() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempdir().unwrap();
    let fake = tmp.path().join("gh-fixture");
    std::fs::write(&fake,b"#!/bin/sh\ntest \"$*\" = 'secret set APPLE_CERTIFICATE_PASSWORD --repo github.com/demo/test --env Staging' || exit 2\ntest \"$(cat)\" = 'fixture-only' || exit 3\n").unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut d = Document::initial();
    let s = Scope::default();
    let k = "APPLE_CERTIFICATE_PASSWORD";
    let key = Zeroizing::new(vault::random());
    d.save(&s, k, b"fixture-only", "", &key).unwrap();
    assert!(github_with(
        &mut d,
        &s,
        k,
        "demo/test",
        "Other",
        &key,
        Command::new(&fake)
    )
    .is_err());
    assert!(d.entry(&s, k).unwrap().binding.is_none());
    github_with(
        &mut d,
        &s,
        k,
        "demo/test",
        "Staging",
        &key,
        Command::new(&fake),
    )
    .unwrap();
    assert_eq!(
        d.entry(&s, k).unwrap().binding.as_ref().unwrap().repo,
        "demo/test"
    );
}

#[test]
fn shared_credentials_isolate_remote_bindings_and_protect_owner() {
    let key = Zeroizing::new(vault::random());
    let mut doc = Document::initial();
    let source = Scope::default();
    let dest = Scope {
        project: doc
            .add_project("Second", vec![source.target.clone()])
            .unwrap(),
        ..source.clone()
    };
    let third = Scope {
        project: doc
            .add_project("Third", vec![source.target.clone()])
            .unwrap(),
        ..source.clone()
    };
    let name = "APPLE_CERTIFICATE";
    doc.save(&source, name, b"original", "", &key).unwrap();
    assert!(doc
        .sources(
            &Scope {
                env: "Development".into(),
                ..dest.clone()
            },
            name
        )
        .is_empty());
    doc.link(&dest, name, source.project).unwrap();
    let local = doc
        .entries
        .get_mut(&Document::address(&dest, name))
        .unwrap();
    assert!(local.cipher.is_empty());
    local.synced = "test/destination".into();
    assert!(doc.local_entry(&source, name).unwrap().synced.is_empty());
    assert_eq!(doc.dependents(&source, name), vec![dest.project]);
    assert!(doc.link(&third, name, dest.project).is_err());
    assert!(doc.clear_value(&source, name).is_err());
    assert!(doc.save(&dest, name, b"overwrite", "", &key).is_err());
    doc.save(&source, name, b"rotated", "", &key).unwrap();
    assert!(doc.local_entry(&dest, name).unwrap().synced.is_empty());
    let restored: Document = serde_json::from_slice(&serde_json::to_vec(&doc).unwrap()).unwrap();
    assert_eq!(
        value(&restored, &dest, name, &key).unwrap().as_slice(),
        b"rotated"
    );
    doc.unlink(&dest, name).unwrap();
    assert!(doc.entry(&dest, name).is_err());
    assert_eq!(
        value(&doc, &source, name, &key).unwrap().as_slice(),
        b"rotated"
    );
    doc.clear_value(&source, name).unwrap();
}

#[test]
fn certificates_compare_actual_public_keys_and_replace_old_evidence() {
    use openssl::{
        asn1::Asn1Time,
        bn::BigNum,
        hash::MessageDigest,
        pkcs12::Pkcs12,
        pkey::PKey,
        rsa::Rsa,
        x509::{X509NameBuilder, X509},
    };
    let private = PKey::from_rsa(Rsa::generate(2048).unwrap()).unwrap();
    let other = PKey::from_rsa(Rsa::generate(2048).unwrap()).unwrap();
    let mut name = X509NameBuilder::new().unwrap();
    name.append_entry_by_text("CN", "ReleaseVault fixture")
        .unwrap();
    let name = name.build();
    let certificate = |pk: &openssl::pkey::PKeyRef<openssl::pkey::Private>| {
        let mut cert = X509::builder().unwrap();
        cert.set_version(2).unwrap();
        cert.set_serial_number(&BigNum::from_u32(1).unwrap().to_asn1_integer().unwrap())
            .unwrap();
        cert.set_subject_name(&name).unwrap();
        cert.set_issuer_name(&name).unwrap();
        cert.set_pubkey(pk).unwrap();
        cert.set_not_before(&Asn1Time::days_from_now(0).unwrap())
            .unwrap();
        cert.set_not_after(&Asn1Time::days_from_now(30).unwrap())
            .unwrap();
        cert.sign(pk, MessageDigest::sha256()).unwrap();
        cert.build()
    };
    let cert = certificate(&private);
    let mismatch = certificate(&other);
    let p12 = Pkcs12::builder()
        .name("fixture")
        .pkey(&private)
        .cert(&cert)
        .build2("file-password")
        .unwrap();
    let mut doc = Document::initial();
    let key = Zeroizing::new(vault::random());
    let scope = Scope::default();
    let name = "APPLE_CERTIFICATE";
    doc.save(
        &scope,
        name,
        STANDARD.encode(p12.to_der().unwrap()).as_bytes(),
        "",
        &key,
    )
    .unwrap();
    assert!(certificates::match_certificate(
        &mut doc,
        &scope,
        name,
        &cert.to_der().unwrap(),
        "wrong",
        &key
    )
    .is_err());
    assert_eq!(
        certificates::match_certificate(
            &mut doc,
            &scope,
            name,
            &mismatch.to_der().unwrap(),
            "file-password",
            &key
        ),
        Err("key-mismatch".into())
    );
    assert!(doc
        .entry(&scope, name)
        .unwrap()
        .matched_fingerprint
        .is_empty());
    certificates::match_certificate(
        &mut doc,
        &scope,
        name,
        &cert.to_der().unwrap(),
        "file-password",
        &key,
    )
    .unwrap();
    assert_eq!(
        doc.entry(&scope, name).unwrap().matched_fingerprint.len(),
        95
    );
    doc.save(
        &scope,
        name,
        &private.private_key_to_pem_pkcs8().unwrap(),
        "",
        &key,
    )
    .unwrap();
    assert!(doc
        .entry(&scope, name)
        .unwrap()
        .matched_fingerprint
        .is_empty());
    certificates::match_certificate(&mut doc, &scope, name, &cert.to_pem().unwrap(), "", &key)
        .unwrap();
}

#[test]
fn device_handles_stay_local_and_old_documents_open() {
    let tmp = tempdir().unwrap();
    let mut v = Vault::open(&tmp.path().join("source")).unwrap();
    v.create(P).unwrap();
    let local = b"machine-bound-test-record";
    v.set_method_record(Method::Mac, Some(local)).unwrap();
    let portable = v.backup(P).unwrap();
    assert!(!portable.windows(local.len()).any(|w| w == local));
    let mut second = Vault::open(&tmp.path().join("restored")).unwrap();
    second.restore(&portable, P).unwrap();
    assert!(!second.method_enabled(Method::Mac).unwrap());
    let key = v.unlocked().unwrap().key.clone();
    v.session = None;
    assert!(v
        .finish_method_unlock(Method::Mac, b"different", key.clone())
        .is_err());
    assert!(v
        .finish_method_unlock(Method::Mac, local, Zeroizing::new(vault::random()))
        .is_err());
    assert!(v.session.is_none());
    v.finish_method_unlock(Method::Mac, local, key.clone())
        .unwrap();
    v.set_method_record(Method::Mac, None).unwrap();
    assert!(v
        .finish_method_unlock(Method::Mac, local, key.clone())
        .is_err());
    let s = Scope::default();
    let key_name = "APPLE_TEAM_ID";
    let mut d = Document::initial();
    d.save(&s, key_name, b"legacy", "", &key).unwrap();
    let mut before = serde_json::to_value(d).unwrap();
    let entry = before["entries"][Document::address(&s, key_name)]
        .as_object_mut()
        .unwrap();
    entry.remove("owner");
    entry.remove("matched_fingerprint");
    let restored: Document = serde_json::from_value(before).unwrap();
    assert_eq!(
        value(&restored, &s, key_name, &key).unwrap().as_slice(),
        b"legacy"
    );
}

#[test]
fn stale_session_cannot_start_remote_side_effects() {
    let tmp = tempdir().unwrap();
    let path = tmp.path().join("vault.sqlite");
    let mut first = Vault::open(&path).unwrap();
    first.create(P).unwrap();
    let mut second = Vault::open(&path).unwrap();
    second.unlock(P).unwrap();
    first
        .change(|d, _| {
            d.event(&Scope::default(), "", "changed");
            Ok(())
        })
        .unwrap();
    let stale = second.change::<()>(|_, _| panic!("stale callback ran"));
    assert_eq!(stale.unwrap_err(), "conflict");
}

#[test]
fn apple_account_fields_preserve_and_protect_legacy_custom_values() {
    let tmp = tempdir().unwrap();
    let path = tmp.path().join("legacy.sqlite");
    let mut vault = Vault::open(&path).unwrap();
    vault.create(P).unwrap();
    let s = Scope::default();
    vault
        .change(|d, k| {
            // The previous catalog allowed a user-defined APPLE_PASSWORD on any target.
            let mut legacy = d.def(&s, "APPLE_PASSWORD")?.clone();
            legacy.kind = "config".into();
            legacy.targets = vec![s.target.clone(), "windows-direct".into()];
            d.projects[0].custom.push(legacy);
            d.projects[0].targets.push("windows-direct".into());
            d.save(&s, "APPLE_ACCOUNT_PASSWORD", b"test-only-sign-in", "", k)?;
            d.save_app_password(&s, b"test-only-app-password", "first label", "", k)?;
            d.save_app_password(&s, b"", "notarization", "", k)?;
            record_sent(d, &s, "APPLE_PASSWORD", "fixture/sample", "", None)?;
            d.save(&s, "APPLE_ID", b"sample@example.test", "", k)
        })
        .unwrap();
    drop(vault);
    let mut vault = Vault::open(&path).unwrap();
    vault.unlock(P).unwrap();
    let session = vault.unlocked().unwrap();
    for target in ["mac-direct", "mac-store", "windows-direct"] {
        let scope = Scope {
            target: target.into(),
            ..s.clone()
        };
        let defs = session.doc.defs(&scope).unwrap();
        assert_eq!(defs.iter().filter(|d| d.key == "APPLE_PASSWORD").count(), 1);
        assert_eq!(
            session.doc.def(&scope, "APPLE_PASSWORD").unwrap().kind,
            "secret"
        );
        assert_eq!(
            &**value(&session.doc, &scope, "APPLE_PASSWORD", &session.key).unwrap(),
            b"test-only-app-password"
        );
    }
    assert_eq!(
        session.doc.def(&s, "APPLE_ACCOUNT_PASSWORD").unwrap().kind,
        "secret"
    );
    assert_eq!(
        &**value(&session.doc, &s, "APPLE_ACCOUNT_PASSWORD", &session.key).unwrap(),
        b"test-only-sign-in"
    );
    let entry = session.doc.local_entry(&s, "APPLE_PASSWORD").unwrap();
    assert_eq!(entry.app_password_label, "notarization");
    assert!(entry.binding.is_none() && entry.synced.is_empty());
    assert_eq!(session.doc.def(&s, "APPLE_ID").unwrap().kind, "secret");
    assert_eq!(
        &**value(&session.doc, &s, "APPLE_ID", &session.key).unwrap(),
        b"sample@example.test"
    );
}

#[test]
fn bundled_guides_cover_every_credential() {
    let data: Value = serde_json::from_str(include_str!("../../resources/sources.json")).unwrap();
    let catalog = model::catalog();
    assert_eq!(catalog.len(), data["items"].as_object().unwrap().len());
    for def in catalog {
        let docs = guide_url(def, false).unwrap();
        let get = guide_url(def, true).unwrap();
        assert!(
            docs.starts_with("https://") && get.starts_with("https://"),
            "{}",
            def.key
        );
        assert_ne!(docs, get, "{}", def.key);
        let translations = data["items"][&def.key]["purpose"].as_array().unwrap();
        assert_eq!(translations.len(), 3);
        assert!(translations
            .iter()
            .all(|v| v.as_str().is_some_and(|s| !s.trim().is_empty())));
    }
}

#[test]
fn passkey_handles_are_independent_revocable_and_not_restored() {
    let tmp = tempdir().unwrap();
    let db = tmp.path().join("source");
    let mut v = Vault::open(&db).unwrap();
    v.create(P).unwrap();
    let key = v.unlocked().unwrap().key.clone();
    v.set_method_record(Method::Mac, Some(b"mac")).unwrap();
    v.set_method_record(Method::Passkey, Some(b"passkey"))
        .unwrap();
    let backup = v.backup(P).unwrap();
    drop(v);
    let mut restored = Vault::open(&tmp.path().join("restore")).unwrap();
    restored.restore(&backup, P).unwrap();
    assert!(!restored.method_enabled(Method::Passkey).unwrap());
    let mut v = Vault::open(&db).unwrap();
    assert!(v
        .finish_method_unlock(Method::Passkey, b"mac", key.clone())
        .is_err());
    assert!(v
        .finish_method_unlock(Method::Passkey, b"passkey", Zeroizing::new(vault::random()))
        .is_err());
    assert!(v.session.is_none());
    v.finish_method_unlock(Method::Passkey, b"passkey", key.clone())
        .unwrap();
    v.set_method_record(Method::Passkey, None).unwrap();
    v.session = None;
    assert!(v
        .finish_method_unlock(Method::Passkey, b"passkey", key.clone())
        .is_err());
    assert!(v.method_enabled(Method::Mac).unwrap());
    v.unlock(P).unwrap();
}

#[test]
fn derived_app_id_preserves_prefix_across_edits_and_restart() {
    let tmp = tempdir().unwrap();
    let path = tmp.path().join("app-id.sqlite");
    let mut vault = Vault::open(&path).unwrap();
    vault.create(P).unwrap();
    let scope = Scope::default();
    let staging = Scope {
        env: "Staging".into(),
        ..scope.clone()
    };
    let save = |vault: &mut Vault, scope: &Scope, field: &str, value: &str| {
        vault
            .change(|d, k| d.save(scope, field, value.as_bytes(), "", k))
            .unwrap();
    };
    let id = |vault: &mut Vault, scope: &Scope| {
        let session = vault.unlocked().unwrap();
        value(&session.doc, scope, apple_ids::APP_ID, &session.key)
            .ok()
            .map(|bytes| String::from_utf8(bytes.to_vec()).unwrap())
    };
    save(&mut vault, &scope, "APPLE_BUNDLE_ID", "com.example.app");
    assert_eq!(id(&mut vault, &scope), None);
    save(&mut vault, &scope, "APPLE_TEAM_ID", "TEAM123456");
    assert_eq!(
        id(&mut vault, &scope).as_deref(),
        Some("TEAM123456.com.example.app")
    );
    save(
        &mut vault,
        &scope,
        apple_ids::APP_ID,
        "OLDP123456.com.example.app",
    );
    drop(vault);
    let mut vault = Vault::open(&path).unwrap();
    vault.unlock(P).unwrap();
    save(&mut vault, &scope, "APPLE_TEAM_ID", "TEAM654321");
    save(&mut vault, &scope, "APPLE_BUNDLE_ID", "com.example.new");
    assert_eq!(
        id(&mut vault, &scope).as_deref(),
        Some("OLDP123456.com.example.new")
    );
    save(
        &mut vault,
        &scope,
        apple_ids::APP_ID,
        "TEAM654321.com.example.new",
    );
    save(&mut vault, &scope, "APPLE_TEAM_ID", "TEAM123456");
    assert_eq!(
        id(&mut vault, &scope).as_deref(),
        Some("TEAM123456.com.example.new")
    );
    assert_eq!(id(&mut vault, &staging), None);
    vault
        .change(|d, _| d.clear_value(&scope, "APPLE_TEAM_ID"))
        .unwrap();
    assert_eq!(id(&mut vault, &scope), None);
    save(
        &mut vault,
        &staging,
        apple_ids::APP_ID,
        "MANUAL0001.com.example.old",
    );
    save(&mut vault, &staging, "APPLE_TEAM_ID", "TEAM123456");
    save(&mut vault, &staging, "APPLE_BUNDLE_ID", "com.example.new");
    assert_eq!(
        id(&mut vault, &staging).as_deref(),
        Some("MANUAL0001.com.example.old")
    );
}

#[test]
fn multi_os_workflow_selects_only_configured_targets_and_isolates_artifacts() {
    let mut d = Document::initial();
    let s = Scope::default();
    let all = [
        "darwin-aarch64",
        "darwin-x86_64",
        "windows-x86_64",
        "linux-x86_64",
    ]
    .map(String::from);
    assert!(publishing::project_workflow(&d, &s, &all).is_err());
    assert!(publishing::project_workflow(&d, &s, &[]).is_err());
    assert!(publishing::project_workflow(&d, &s, &["bad\n    run: injected".into()]).is_err());
    d.projects[0]
        .targets
        .extend(["windows-exe".into(), "linux-appimage".into()]);
    let yaml = publishing::project_workflow(&d, &s, &all).unwrap();
    assert_eq!(yaml.matches("      - uses: actions/checkout@v4").count(), 4);
    assert_eq!(yaml.matches("environment: Production").count(), 4);
    assert_eq!(
        yaml.matches("          APPLE_PASSWORD: ${{ secrets.APPLE_PASSWORD }}")
            .count(),
        2
    );
    assert!(!yaml.contains("APPLE_ACCOUNT_PASSWORD"));
    assert!(!yaml.contains("contents: write"));
    assert!(yaml.contains("permissions:\n  contents: read"));
    for platform in &all {
        assert!(yaml.contains(&format!("name: updater-{platform}")));
        assert!(yaml.contains(&format!("      - build_{}", platform.replace('-', "_"))));
    }
    assert!(yaml.contains("merge-multiple: false"));
    assert!(yaml.contains("No signed updater artifacts were generated"));
    assert!(yaml.contains("createUpdaterArtifacts:true"));
    assert!(publishing::project_workflow(&d, &s, &[all[0].clone(), all[0].clone()]).is_err());
    let invalid = Scope {
        env: "Production\nrun: injected".into(),
        ..s.clone()
    };
    assert!(publishing::project_workflow(&d, &invalid, &all).is_err());
    if let Ok(file) = std::env::var("RV_WORKFLOW_QA") {
        std::fs::write(file, &yaml).unwrap();
    }
}

#[test]
#[cfg(unix)]
fn workflow_cli_preserves_http_status_for_missing_file() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempdir().unwrap();
    let script = tmp.path().join("fake-gh");
    std::fs::write(
        &script,
        br#"#!/bin/sh
printf 'HTTP/2.0 404 Not Found\r\nContent-Type: application/json\r\n\r\n{"message":"not found"}\n'
exit 1
"#,
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
    let (status, body) = workflow_cli_with(
        Command::new(script),
        "GET",
        "/repos/example/test/contents/test",
        None,
    )
    .unwrap();
    assert_eq!(status, 404);
    assert_eq!(body["message"], "not found");
}

#[test]
fn expiry_overview_groups_shared_entries_and_keeps_metadata_only() {
    let key = Zeroizing::new(vault::random());
    let mut d = Document::initial();
    let today = chrono::NaiveDate::from_ymd_opt(2026, 11, 6).unwrap();
    let s = Scope::default();
    d.save(
        &s,
        "APPLE_CERTIFICATE",
        b"private-value-must-stay-in-rust",
        "2026-11-05",
        &key,
    )
    .unwrap();
    let receiver = d
        .add_project("Receiver", vec!["mac-direct".into()])
        .unwrap();
    let linked = Scope {
        project: receiver,
        ..s.clone()
    };
    d.link(&linked, "APPLE_CERTIFICATE", s.project).unwrap();
    let staging = Scope {
        env: "Staging".into(),
        ..s.clone()
    };
    d.save(&staging, "APPLE_CERTIFICATE", b"other", "2026-12-06", &key)
        .unwrap();
    let store = Scope {
        target: "mac-store".into(),
        ..s.clone()
    };
    d.save(
        &store,
        "APPLE_PROVISIONING_PROFILE",
        b"profile",
        "2030-01-01",
        &key,
    )
    .unwrap();
    d.entries
        .get_mut(&Document::address(&store, "APPLE_PROVISIONING_PROFILE"))
        .unwrap()
        .apple_check = Some(apple_api::Evidence {
        kind: "profiles".into(),
        id: "profile-1".into(),
        name: "Test profile".into(),
        checked: "2026-11-06T00:00:00Z".into(),
        state: "INVALID".into(),
    });
    d.save(&s, "APPLE_TEAM_ID", b"TEAM123456", "", &key)
        .unwrap();
    let report = expiry::overview(&d, &linked, today);
    assert_eq!(report.items.len(), 3);
    assert_eq!(report.items[0].state, expiry::State::Invalid);
    let expired = report
        .items
        .iter()
        .find(|item| item.flags.contains(&expiry::State::Expired))
        .unwrap();
    assert_eq!(expired.days, Some(-1));
    assert_eq!(expired.projects.len(), 2);
    assert_eq!(expired.scope.project, s.project);
    assert!(expired.flags.contains(&expiry::State::Unchecked));
    let soon = report
        .items
        .iter()
        .find(|item| item.scope.env == "Staging")
        .unwrap();
    assert_eq!(soon.days, Some(30));
    assert!(soon.flags.contains(&expiry::State::Soon));
    let json = serde_json::to_string(&report).unwrap();
    assert!(!json.contains("private-value-must-stay-in-rust"));
    assert!(!json.contains("cipher"));
    assert!(!report
        .items
        .iter()
        .any(|item| item.def.key == "APPLE_TEAM_ID"));
    d.custom(&s, "CUSTOM_EXPIRING", "Reference token", "secret")
        .unwrap();
    d.custom(&linked, "CUSTOM_EXPIRING", "Source token", "secret")
        .unwrap();
    d.save(
        &linked,
        "CUSTOM_EXPIRING",
        b"custom-secret",
        "2026-11-07",
        &key,
    )
    .unwrap();
    d.link(&s, "CUSTOM_EXPIRING", linked.project).unwrap();
    let report = expiry::overview(&d, &s, today);
    let custom = report
        .items
        .iter()
        .find(|item| item.def.key == "CUSTOM_EXPIRING")
        .unwrap();
    assert_eq!(custom.def.name, "Source token");
    assert_eq!(custom.source_id, linked.project);
    assert_eq!(custom.scope.project, linked.project);
    assert_eq!(custom.projects.len(), 2);
}
#[test]
fn expiry_overview_handles_today_and_unset_dates() {
    let key = Zeroizing::new(vault::random());
    let mut d = Document::initial();
    let s = Scope::default();
    let today = chrono::NaiveDate::from_ymd_opt(2026, 11, 6).unwrap();
    d.save(&s, "APPLE_CERTIFICATE", b"opaque", "", &key)
        .unwrap();
    let report = expiry::overview(&d, &s, today);
    assert_eq!(
        report.items[0].flags,
        vec![expiry::State::Unchecked, expiry::State::Unknown]
    );
    d.save(&s, "APPLE_CERTIFICATE", b"opaque", "2026-11-06", &key)
        .unwrap();
    assert_eq!(expiry::overview(&d, &s, today).items[0].days, Some(0));
    assert!(expiry::overview(&d, &s, today).items[0]
        .flags
        .contains(&expiry::State::Soon));
    d.save(&s, "APPLE_CERTIFICATE", b"opaque", "2026-12-07", &key)
        .unwrap();
    let entry = d
        .entries
        .get_mut(&Document::address(&s, "APPLE_CERTIFICATE"))
        .unwrap();
    entry.apple_check = Some(apple_api::Evidence {
        kind: "certificates".into(),
        id: "cert".into(),
        name: "cert".into(),
        checked: "2026-11-06T00:00:00Z".into(),
        state: "PRESENT".into(),
    });
    assert_eq!(
        expiry::overview(&d, &s, today).items[0].state,
        expiry::State::Valid
    );
}
