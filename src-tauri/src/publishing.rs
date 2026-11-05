//! Produce files for review; no repository or release is changed here.
use crate::{
    integrations::{verify_artifact, PRIVATE},
    model::{Document, Scope},
    vault::{Key, R},
};
use serde_json::json;
use std::path::Path;
pub fn manifest(
    d: &Document,
    s: &Scope,
    key: &Key,
    artifact: &Path,
    sig: &[u8],
    version: &str,
    platform: &str,
    url: &str,
) -> R<Vec<u8>> {
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
    verify_artifact(d, s, artifact, sig, key)?;
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
    if runner.starts_with("macos") {
        y.push_str("      - name: Prepare notarization key\n        shell: bash\n        env:\n          P8: ${{ secrets.APPLE_API_KEY_P8 }}\n        run: |\n          umask 077\n          printf '%s' \"$P8\" > \"$RUNNER_TEMP/notary.p8\"\n          echo \"APPLE_API_KEY_PATH=$RUNNER_TEMP/notary.p8\" >> \"$GITHUB_ENV\"\n");
    }
    y.push_str("      - name: Build application and updater artifacts\n        run: npm exec -- tauri build\n        env:\n");
    for k in [PRIVATE, "TAURI_SIGNING_PRIVATE_KEY_PASSWORD"] {
        y.push_str(&format!("          {k}: ${{{{ secrets.{k} }}}}\n"))
    }
    if runner.starts_with("macos") {
        for k in [
            "APPLE_CERTIFICATE",
            "APPLE_CERTIFICATE_PASSWORD",
            "APPLE_SIGNING_IDENTITY",
            "APPLE_API_ISSUER",
            "APPLE_API_KEY",
        ] {
            let ty = if k.starts_with("APPLE_CERTIFICATE") {
                "secrets"
            } else {
                "vars"
            };
            y.push_str(&format!("          {k}: ${{{{ {ty}.{k} }}}}\n"))
        }
        y.push_str("      - if: always()\n        shell: bash\n        run: rm -f \"$RUNNER_TEMP/notary.p8\"\n");
    }
    y.push_str("      - uses: actions/upload-artifact@v4\n        with:\n          name: updater-artifacts\n          path: src-tauri/target/release/bundle/\n          if-no-files-found: error\n");
    Ok(y)
}
