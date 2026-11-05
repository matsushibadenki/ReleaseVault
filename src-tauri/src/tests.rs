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
    let byte = broken["body"][35].as_u64().unwrap();
    broken["body"][35] = json!(byte ^ 1);
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
        .args(["signer", "sign"])
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
