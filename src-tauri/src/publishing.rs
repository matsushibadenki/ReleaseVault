//! Produce files for review; no repository or release is changed here.
use crate::{
    integrations::{verify_artifact, PRIVATE},
    model::{Document, Scope},
    vault::{Key, R},
};
use serde_json::json;
use std::path::Path;
pub struct ReleaseInfo<'a> {
    pub version: &'a str,
    pub platform: &'a str,
    pub url: &'a str,
}
pub fn manifest(
    d: &Document,
    s: &Scope,
    key: &Key,
    artifact: &Path,
    sig: &[u8],
    release: ReleaseInfo,
) -> R<Vec<u8>> {
    let ReleaseInfo {
        version,
        platform,
        url,
    } = release;
    let _ = semver::Version::parse(version).map_err(|_| "invalid")?;
    let os = match s.target.as_str() {
        "mac-direct" => "darwin",
        "windows-direct" | "windows-exe" => "windows",
        "linux-appimage" => "linux",
        _ => return Err("scope".into()),
    };
    if !["x86_64", "aarch64", "i686"]
        .iter()
        .any(|arch| platform == format!("{os}-{arch}"))
    {
        return Err("invalid".into());
    }
    let parsed = tauri::Url::parse(url).map_err(|_| "invalid")?;
    if parsed.scheme() != "https"
        || parsed.host().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
    {
        return Err("invalid".into());
    }
    if verify_artifact(d, s, artifact, sig, key)?.as_deref() != Some(version) {
        return Err("version".into());
    }
    serde_json::to_vec_pretty(&json!({"version":version,"pub_date":chrono::Utc::now().to_rfc3339(),"platforms":{(platform):{"signature":std::str::from_utf8(sig.trim_ascii()).map_err(|_|"format")?,"url":url}}})).map_err(|_|"format".into())
}
pub fn workflow(s: &Scope) -> R<String> {
    let runner = match s.target.as_str() {
        "mac-direct" => "macos-latest",
        "windows-direct" | "windows-exe" => "windows-latest",
        "linux-appimage" => "ubuntu-24.04",
        _ => return Err("scope".into()),
    };
    if !crate::model::ENVS.contains(&s.env.as_str()) {
        return Err("scope".into());
    }
    let mut y=format!("name: Build signed updater artifacts\non: workflow_dispatch\npermissions:\n  contents: read\njobs:\n  build:\n    runs-on: {runner}\n    environment: {}\n    steps:\n      - uses: actions/checkout@v4\n      - uses: actions/setup-node@v4\n        with:\n          node-version: '22'\n",s.env);
    if runner.starts_with("ubuntu") {
        y.push_str("      - name: Install native dependencies\n        run: sudo apt-get update && sudo apt-get install -y libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf libssl-dev\n");
    }
    y.push_str("      - run: npm ci\n");
    y.push_str("      - name: Build application and updater artifacts\n");
    if runner.starts_with("macos") {
        y.push_str(r#"        shell: bash
        run: |
          set -eu
          if [[ -n "$P8" ]]; then
            : "${APPLE_API_KEY:?Missing APPLE_API_KEY}" "${APPLE_API_ISSUER:?Missing APPLE_API_ISSUER}"
            unset APPLE_ID APPLE_PASSWORD
            export APPLE_API_KEY_PATH="$RUNNER_TEMP/notary.p8"
            trap 'rm -f "$APPLE_API_KEY_PATH"' EXIT
            (umask 077; printf '%s' "$P8" > "$APPLE_API_KEY_PATH")
          elif [[ -n "$APPLE_ID" || -n "$APPLE_PASSWORD" ]]; then
            : "${APPLE_ID:?Missing APPLE_ID}" "${APPLE_PASSWORD:?Missing APPLE_PASSWORD}" "${APPLE_TEAM_ID:?Missing APPLE_TEAM_ID}"
            unset APPLE_API_KEY APPLE_API_ISSUER
          elif [[ -n "$APPLE_API_KEY" || -n "$APPLE_API_ISSUER" ]]; then
            echo "Missing APPLE_API_KEY_P8" >&2
            exit 1
          else
            unset APPLE_ID APPLE_PASSWORD APPLE_API_KEY APPLE_API_ISSUER
          fi
          unset P8
          npm exec -- tauri build
"#);
    } else {
        y.push_str("        run: npm exec -- tauri build\n");
    }
    y.push_str("        env:\n");
    for k in [PRIVATE, "TAURI_SIGNING_PRIVATE_KEY_PASSWORD"] {
        y.push_str(&format!("          {k}: ${{{{ secrets.{k} }}}}\n"))
    }
    if runner.starts_with("macos") {
        y.push_str("          P8: ${{ secrets.APPLE_API_KEY_P8 }}\n");
        for (context, keys) in [
            (
                "secrets",
                [
                    "APPLE_CERTIFICATE",
                    "APPLE_CERTIFICATE_PASSWORD",
                    "APPLE_ID",
                    "APPLE_PASSWORD",
                ],
            ),
            (
                "vars",
                [
                    "APPLE_SIGNING_IDENTITY",
                    "APPLE_API_ISSUER",
                    "APPLE_API_KEY",
                    "APPLE_TEAM_ID",
                ],
            ),
        ] {
            for key in keys {
                y.push_str(&format!("          {key}: ${{{{ {context}.{key} }}}}\n"))
            }
        }
    }
    y.push_str("      - uses: actions/upload-artifact@v4\n        with:\n          name: updater-artifacts\n          path: src-tauri/target/release/bundle/\n          if-no-files-found: error\n");
    Ok(y)
}
