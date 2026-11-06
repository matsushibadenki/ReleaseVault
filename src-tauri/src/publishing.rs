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
#[derive(Clone, Copy)]
struct BuildTarget {
    platform: &'static str,
    distribution: &'static str,
    runner: &'static str,
    triple: &'static str,
}
const BUILD_TARGETS: [BuildTarget; 4] = [
    BuildTarget {
        platform: "darwin-aarch64",
        distribution: "mac-direct",
        runner: "macos-15",
        triple: "aarch64-apple-darwin",
    },
    BuildTarget {
        platform: "darwin-x86_64",
        distribution: "mac-direct",
        runner: "macos-15-intel",
        triple: "x86_64-apple-darwin",
    },
    BuildTarget {
        platform: "windows-x86_64",
        distribution: "windows-direct",
        runner: "windows-2025",
        triple: "x86_64-pc-windows-msvc",
    },
    BuildTarget {
        platform: "linux-x86_64",
        distribution: "linux-appimage",
        runner: "ubuntu-24.04",
        triple: "x86_64-unknown-linux-gnu",
    },
];
pub fn workflow(s: &Scope) -> R<String> {
    let platform = match s.target.as_str() {
        "mac-direct" => "darwin-aarch64",
        "windows-direct" | "windows-exe" => "windows-x86_64",
        "linux-appimage" => "linux-x86_64",
        _ => return Err("scope".into()),
    };
    build_workflow(
        &s.env,
        &[BUILD_TARGETS
            .iter()
            .find(|t| t.platform == platform)
            .copied()
            .ok_or("scope")?],
    )
}
pub fn project_workflow(d: &Document, s: &Scope, platforms: &[String]) -> R<String> {
    let project = d.project(s)?;
    if platforms.is_empty() || platforms.len() > BUILD_TARGETS.len() {
        return Err("scope".into());
    }
    let mut selected = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for platform in platforms {
        let target = BUILD_TARGETS
            .iter()
            .find(|t| t.platform == platform)
            .ok_or("scope")?;
        let allowed = project.targets.iter().any(|distribution| {
            distribution == target.distribution
                || (target.distribution == "windows-direct" && distribution == "windows-exe")
        });
        if !allowed || !seen.insert(platform) {
            return Err("scope".into());
        }
        selected.push(*target);
    }
    build_workflow(&s.env, &selected)
}
fn build_workflow(environment: &str, targets: &[BuildTarget]) -> R<String> {
    if !crate::model::ENVS.contains(&environment) {
        return Err("scope".into());
    }
    let mut yaml=String::from("name: Build signed updater artifacts\non: workflow_dispatch\npermissions:\n  contents: read\njobs:\n");
    for target in targets {
        yaml.push_str(&build_job(environment, *target));
    }
    yaml.push_str(
        "  collect:\n    name: Collect updater artifacts\n    runs-on: ubuntu-24.04\n    needs:\n",
    );
    for target in targets {
        yaml.push_str(&format!(
            "      - build_{}\n",
            target.platform.replace('-', "_")
        ));
    }
    yaml.push_str(
        r#"    steps:
      - uses: actions/download-artifact@v4
        with:
          pattern: updater-*
          path: collected
          merge-multiple: false
      - uses: actions/upload-artifact@v4
        with:
          name: release-bundle-${{ github.run_number }}-${{ github.run_attempt }}
          path: collected/
          if-no-files-found: error
          compression-level: 0
          retention-days: 14
"#,
    );
    Ok(yaml)
}
fn build_job(environment: &str, target: BuildTarget) -> String {
    let job_id = target.platform.replace('-', "_");
    let mut yaml = format!(
        r#"  build_{job_id}:
    name: Build {platform}
    runs-on: {runner}
    timeout-minutes: 60
    environment: {environment}
    env:
      CARGO_TARGET_DIR: ${{{{ github.workspace }}}}/target
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '22'
      - name: Set up Rust
        shell: bash
        run: |
          rustup toolchain install stable --profile minimal
          rustup default stable
          rustup target add {triple}
"#,
        platform = target.platform,
        runner = target.runner,
        triple = target.triple,
    );
    if target.distribution == "linux-appimage" {
        yaml.push_str("      - name: Install native dependencies\n        run: sudo apt-get update && sudo apt-get install -y libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf libssl-dev\n");
    }
    yaml.push_str(r#"      - run: npm ci
      - name: Enable updater artifacts
        shell: bash
        run: node -e 'require("fs").writeFileSync("releasevault.updater.json",JSON.stringify({bundle:{createUpdaterArtifacts:true}}))'
      - name: Build application and updater artifacts
"#);
    let command = format!(
        "npm exec -- tauri build --target {} --config releasevault.updater.json",
        target.triple
    );
    if target.distribution == "mac-direct" {
        let script = r#"        shell: bash
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
          __BUILD_COMMAND__
"#;
        yaml.push_str(&script.replace("__BUILD_COMMAND__", &command));
    } else {
        yaml.push_str(&format!("        shell: bash\n        run: {command}\n"));
    }
    yaml.push_str("        env:\n");
    for field in [PRIVATE, "TAURI_SIGNING_PRIVATE_KEY_PASSWORD"] {
        yaml.push_str(&format!("          {field}: ${{{{ secrets.{field} }}}}\n"));
    }
    if target.distribution == "mac-direct" {
        yaml.push_str("          P8: ${{ secrets.APPLE_API_KEY_P8 }}\n");
        for (context, fields) in [
            (
                "secrets",
                vec![
                    "APPLE_CERTIFICATE",
                    "APPLE_CERTIFICATE_PASSWORD",
                    "APPLE_ID",
                    "APPLE_PASSWORD",
                ],
            ),
            (
                "vars",
                vec![
                    "APPLE_SIGNING_IDENTITY",
                    "APPLE_API_ISSUER",
                    "APPLE_API_KEY",
                    "APPLE_TEAM_ID",
                ],
            ),
        ] {
            for field in fields {
                yaml.push_str(&format!(
                    "          {field}: ${{{{ {context}.{field} }}}}\n"
                ));
            }
        }
    }
    yaml.push_str(&format!(
        r#"      - name: Check updater signature files
        shell: bash
        env:
          RV_BUNDLE_DIR: target/{triple}/release/bundle
        run: |
          node <<'NODE'
          const fs = require('fs');
          const path = require('path');
          function files(dir) {{
            return fs.readdirSync(dir, {{ withFileTypes: true }}).flatMap(entry => {{
              const name = path.join(dir, entry.name);
              return entry.isDirectory() ? files(name) : [name];
            }});
          }}
          const signatures = files(process.env.RV_BUNDLE_DIR).filter(file => file.endsWith('.sig'));
          if (!signatures.length) throw new Error('No signed updater artifacts were generated');
          for (const signature of signatures) {{
            if (!fs.statSync(signature).size || !fs.existsSync(signature.slice(0, -4))) {{
              throw new Error('Missing updater artifact or empty signature');
            }}
          }}
          NODE
      - uses: actions/upload-artifact@v4
        with:
          name: updater-{platform}
          overwrite: true
          path: target/{triple}/release/bundle/
          if-no-files-found: error
          compression-level: 0
          retention-days: 14
"#,
        triple = target.triple,
        platform = target.platform
    ));
    yaml
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct VerifiedPlatform {
    pub target: String,
    pub url: String,
    pub signature: String,
    pub public_key: Vec<u8>,
    pub verified: String,
    #[serde(default)]
    pub remote: Option<RemoteCheck>,
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ReleaseDraft {
    pub version: String,
    pub platforms: std::collections::BTreeMap<String, VerifiedPlatform>,
}
impl ReleaseDraft {
    pub fn address(s: &Scope) -> String {
        format!("{}/{}", s.project, s.env)
    }
    pub fn view(&self, d: &Document, s: &Scope, key: &Key) -> serde_json::Value {
        json!({"version":self.version,"platforms":self.platforms.iter().map(|(platform, entry)| {
            let target=Scope {target:entry.target.clone(),..s.clone()};
            let current=crate::integrations::value(d,&target,crate::integrations::PUBLIC,key).is_ok_and(|public| public.as_slice()==entry.public_key);
            json!({"platform":platform,"target":entry.target,"url":entry.url,"verified":entry.verified,"keyCurrent":current,"remote":entry.remote})
        }).collect::<Vec<_>>()})
    }
}
pub fn add_platform(
    d: &mut Document,
    s: &Scope,
    key: &Key,
    artifact: &Path,
    sig: &[u8],
    release: ReleaseInfo,
) -> R<()> {
    let address = ReleaseDraft::address(s);
    if let Some(draft) = d.release_drafts.get(&address) {
        if draft.version != release.version {
            return Err("release-version".into());
        }
        if draft.platforms.contains_key(release.platform) {
            return Err("release-duplicate".into());
        }
    }
    let bytes = manifest(d, s, key, artifact, sig, ReleaseInfo { ..release })?;
    let json: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| "format")?;
    let entry = VerifiedPlatform {
        target: s.target.clone(),
        url: release.url.into(),
        signature: json["platforms"][release.platform]["signature"]
            .as_str()
            .ok_or("format")?
            .into(),
        public_key: crate::integrations::value(d, s, crate::integrations::PUBLIC, key)?.to_vec(),
        verified: chrono::Utc::now().to_rfc3339(),
        remote: None,
    };
    d.release_drafts
        .entry(address)
        .or_insert_with(|| ReleaseDraft {
            version: release.version.into(),
            platforms: Default::default(),
        })
        .platforms
        .insert(release.platform.into(), entry);
    d.event(s, release.platform, "release-add");
    Ok(())
}
pub fn combined_manifest(d: &Document, s: &Scope, key: &Key) -> R<Vec<u8>> {
    let draft = d
        .release_drafts
        .get(&ReleaseDraft::address(s))
        .ok_or("missing")?;
    if draft.platforms.is_empty() {
        return Err("missing".into());
    }
    let mut platforms = serde_json::Map::new();
    for (platform, entry) in &draft.platforms {
        let scope = Scope {
            target: entry.target.clone(),
            ..s.clone()
        };
        let public = crate::integrations::value(d, &scope, crate::integrations::PUBLIC, key)?;
        if public.as_slice() != entry.public_key {
            return Err("release-key".into());
        }
        platforms.insert(
            platform.clone(),
            json!({"url":entry.url,"signature":entry.signature}),
        );
    }
    serde_json::to_vec_pretty(&json!({"version":draft.version,"pub_date":chrono::Utc::now().to_rfc3339(),"platforms":platforms})).map_err(|_|"format".into())
}
pub fn remove_platform(d: &mut Document, s: &Scope, platform: &str) -> R<()> {
    let address = ReleaseDraft::address(s);
    let draft = d.release_drafts.get_mut(&address).ok_or("missing")?;
    draft.platforms.remove(platform).ok_or("missing")?;
    if draft.platforms.is_empty() {
        d.release_drafts.remove(&address);
    }
    d.event(s, platform, "release-remove");
    Ok(())
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct RemoteCheck {
    pub checked: String,
    pub status: String,
    pub bytes: Option<u64>,
}

pub fn record_remote_check(
    d: &mut Document,
    s: &Scope,
    key: &Key,
    platform: &str,
    version: &str,
    expected: &VerifiedPlatform,
    result: RemoteCheck,
) -> R<()> {
    let draft = d
        .release_drafts
        .get_mut(&ReleaseDraft::address(s))
        .ok_or("missing")?;
    let entry = draft.platforms.get_mut(platform).ok_or("missing")?;
    if draft.version != version
        || entry.url != expected.url
        || entry.signature != expected.signature
        || entry.public_key != expected.public_key
        || entry.verified != expected.verified
    {
        return Err("impact".into());
    }
    let scope = Scope {
        target: entry.target.clone(),
        ..s.clone()
    };
    let public_key = entry.public_key.clone();
    if crate::integrations::value(d, &scope, crate::integrations::PUBLIC, key)?.as_slice()
        != public_key
    {
        return Err("release-key".into());
    }
    d.release_drafts
        .get_mut(&ReleaseDraft::address(s))
        .ok_or("missing")?
        .platforms
        .get_mut(platform)
        .ok_or("missing")?
        .remote = Some(result);
    Ok(())
}
