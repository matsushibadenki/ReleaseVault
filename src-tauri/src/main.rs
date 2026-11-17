#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod actions;
mod apple_api;
mod apple_ids;
mod certificates;
mod device;
mod expiry;
mod github_app;
mod integrations;
mod model;
mod publishing;
mod remote_artifact;
mod sharing;
mod vault;
mod workflow_pr;
use base64::{engine::general_purpose::STANDARD, Engine};
use device::Method;
use model::{Document, Scope};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::{Emitter, Manager, State};
use vault::{Vault, R};
use zeroize::Zeroizing;

struct Workspace {
    vault: Vault,
    scope: Scope,
    selected: String,
    auth_epoch: u64,
}
type Shared = Mutex<Workspace>;
fn notify(app: &tauri::AppHandle) {
    let _ = app.emit("workspace-changed", ());
}
fn panel(app: &tauri::AppHandle) -> bool {
    app.get_webview_window("inspector").is_some()
}
#[tauri::command]
async fn snapshot(app: tauri::AppHandle, state: State<'_, Shared>) -> R<Value> {
    let mut w = state.lock().map_err(|_| "busy")?;
    w.vault.expire();
    let device_enabled = w.vault.method_enabled(Method::Mac)?;
    let passkey_enabled = w.vault.method_enabled(Method::Passkey)?;
    let Some(s) = &w.vault.session else {
        return Ok(
            json!({"locked":true,"passkeyEnabled":passkey_enabled && Method::Passkey.available(),"deviceEnabled":device_enabled && Method::Mac.available(),"exists":w.vault.exists()?}),
        );
    };
    let d = &s.doc;
    let rows = d.defs(&w.scope)?.into_iter().map(|definition| -> R<Value> {
        let entry = d.entry(&w.scope, &definition.key).ok();
        let local=d.local_entry(&w.scope,&definition.key).ok();
        let mut truncated=false;
        let display = match (definition.kind.as_str(),entry) {
            ("secret",_)|(_,None)=>None,
            (_,Some(e))=>{
                let bytes=vault::unseal(&s.key,&e.cipher)?;
                let text=std::str::from_utf8(&bytes).map_err(|_|"format")?;
                let preview:String=text.chars().take(2048).collect();
                truncated=preview.len()!=text.len(); Some(preview)
            }
        };
        Ok(json!({"def":definition,"present":entry.is_some(),"display":display,"appIdPrefix":entry.and_then(|e|e.app_id_prefix.as_ref()).map(|p|match p { apple_ids::AppIdPrefix::Team => "team", apple_ids::AppIdPrefix::Custom(_) => "custom" }),"truncated":truncated,
            "appleCheck":entry.and_then(|e|e.apple_check.as_ref()),"expires":entry.map(|x|&x.expires),"owner":local.and_then(|e|e.owner), "dependents":d.dependents(&w.scope,&definition.key),"sources":d.sources(&w.scope,&definition.key),"appPasswordLabel":entry.map(|e| &e.app_password_label),"fingerprint":entry.map(|e|&e.matched_fingerprint), "synced":local.map(|x|&x.synced),
            "binding":local.and_then(|x|x.binding.as_ref()),"evidence":entry.map(|x|&x.evidence)}))
    }).collect::<R<Vec<_>>>()?;
    Ok(
        json!({"locked":false,"passkeyAvailable":Method::Passkey.available(),"passkeyEnabled":passkey_enabled,"deviceAvailable":Method::Mac.available(),"deviceEnabled":device_enabled,"scope":w.scope,"projects":d.projects,"githubApp":d.github_apps.get(&w.scope.project).map(|c|c.view()),"expiryOverview":expiry::overview(d,&w.scope,chrono::Local::now().date_naive()),"appleReport":d.apple_reports.get(&apple_api::Report::address(&w.scope)).map(|report|report.view(d,&w.scope,&s.key)),"workflowPreview":d.workflow_previews.get(&publishing::ReleaseDraft::address(&w.scope)).map(|p|p.view()),"releaseDraft":d.release_drafts.get(&publishing::ReleaseDraft::address(&w.scope)).map(|draft|draft.view(d,&w.scope,&s.key)),"actionReport":d.action_reports.get(&w.scope.project),"rows":rows,"selected":w.selected,"floating":panel(&app),"audit":d.audit.iter().rev().filter(|e|e.project==w.scope.project&&e.env==w.scope.env).take(150).collect::<Vec<_>>()}),
    )
}
#[tauri::command]
async fn touch(state: State<'_, Shared>) -> R<()> {
    state.lock().map_err(|_| "busy")?.vault.unlocked()?;
    Ok(())
}
#[tauri::command]
async fn authenticate(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
    mode: String,
    password: String,
) -> R<()> {
    let p = Zeroizing::new(password);
    let bytes = if mode == "restore" {
        Some(read(&pick(&["rvbackup"])?, 128 * 1024 * 1024)?)
    } else {
        None
    };
    // Release the workspace mutex while macOS presents its authentication sheet.
    let epoch = {
        let mut w = state.lock().map_err(|_| "busy")?;
        w.auth_epoch += 1;
        w.auth_epoch
    };
    let method = if mode == "passkey" {
        Method::Passkey
    } else {
        Method::Mac
    };
    let device_result = if matches!(mode.as_str(), "device" | "passkey") {
        let record = state
            .lock()
            .map_err(|_| "busy")?
            .vault
            .method_record(method)?;
        Some(
            tauri::async_runtime::spawn_blocking(move || {
                let key = method.open(&record)?;
                Ok::<_, String>((record, key))
            })
            .await
            .map_err(|_| method.error())??,
        )
    } else {
        None
    };
    let mut w = state.lock().map_err(|_| "busy")?;
    if w.auth_epoch != epoch {
        return Err("locked".into());
    }
    match mode.as_str() {
        "create" => w.vault.create(&p)?,
        "unlock" => w.vault.unlock(&p)?,
        "device" | "passkey" => {
            let (record, key) = device_result.ok_or(method.error())?;
            w.vault.finish_method_unlock(method, &record, key)?
        }
        "restore" => w.vault.restore(&bytes.unwrap(), &p)?,
        _ => return Err("invalid".into()),
    }
    let s = w.vault.unlocked()?;
    let project = s.doc.projects.first().ok_or("format")?;
    w.scope = Scope {
        project: project.id,
        target: project.targets.first().ok_or("format")?.clone(),
        env: "Production".into(),
    };
    w.selected = "".into();
    drop(w);
    notify(&app);
    Ok(())
}
#[tauri::command]
async fn lock(app: tauri::AppHandle, state: State<'_, Shared>) -> R<()> {
    let mut w = state.lock().map_err(|_| "busy")?;
    w.auth_epoch += 1;
    w.vault.session = None;
    drop(w);
    notify(&app);
    Ok(())
}
#[tauri::command]
async fn cancel_remote_check(state: State<'_, Shared>) -> R<()> {
    let mut w = state.lock().map_err(|_| "busy")?;
    w.auth_epoch += 1;
    Ok(())
}
#[tauri::command]
async fn select(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
    scope: Scope,
    key: String,
) -> R<()> {
    let mut w = state.lock().map_err(|_| "busy")?;
    let doc = &w.vault.unlocked()?.doc;
    doc.project(&scope)?;
    if !key.is_empty() {
        doc.def(&scope, &key)?;
    }
    w.scope = scope;
    w.selected = key;
    drop(w);
    notify(&app);
    Ok(())
}
#[derive(Default, Deserialize)]
#[serde(default)]
struct Input {
    key: String,
    owner: String,
    affected: Vec<String>,
    key_password: String,
    app_password_label: Option<String>,
    name: String,
    kind: String,
    value: String,
    expires: String,
    password: String,
    repo: String,
    provider: String,
    app_id: String,
    installation_id: String,
    repo_id: String,
    run_id: String,
    preview_id: String,
    report_id: String,
    resource_id: String,
    resource_kind: String,
    #[serde(flatten)]
    grants: github_app::Grants,
    environment: String,
    targets: Vec<String>,
    url: String,
    version: String,
    platform: String,
}
fn pick(filters: &[&str]) -> R<PathBuf> {
    let mut d = rfd::FileDialog::new();
    if !filters.is_empty() {
        d = d.add_filter("ReleaseVault", filters)
    }
    d.pick_file().ok_or_else(|| "cancel".into())
}
fn read(p: &Path, limit: u64) -> R<Zeroizing<Vec<u8>>> {
    let mut b = Zeroizing::new(Vec::new());
    std::fs::File::open(p)
        .map_err(|_| "file")?
        .take(limit + 1)
        .read_to_end(&mut b)
        .map_err(|_| "file")?;
    if b.len() as u64 > limit {
        return Err("size".into());
    }
    Ok(b)
}
fn save(path: &Path, b: &[u8]) -> R<()> {
    let mut f =
        tempfile::NamedTempFile::new_in(path.parent().ok_or("file")?).map_err(|_| "file")?;
    f.write_all(b)
        .and_then(|_| f.as_file().sync_all())
        .map_err(|_| "file")?;
    f.persist(path).map_err(|_| "file")?;
    Ok(())
}
#[tauri::command]
async fn perform(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
    op: String,
    scope: Scope,
    mut input: Input,
) -> R<Value> {
    if input.key == "GITHUB_SSH_KEY_PATH"
        && matches!(
            op.as_str(),
            "github" | "remote" | "watch" | "app-inspect" | "import-file" | "import-text"
        )
    {
        return Err("vault-only".into());
    }
    if input.key == "GITHUB_SSH_KEY_PATH"
        && op == "save"
        && (input.value.contains(['\n', '\r', '\0'])
            || input.value.trim_start().starts_with("-----BEGIN"))
    {
        return Err("invalid".into());
    }
    let pass = Zeroizing::new(std::mem::take(&mut input.password));
    let val = Zeroizing::new(std::mem::take(&mut input.value));
    let key_password = Zeroizing::new(std::mem::take(&mut input.key_password));
    {
        let mut w = state.lock().map_err(|_| "busy")?;
        if w.scope != scope {
            return Err("scope".into());
        }
        w.vault.unlocked()?.doc.project(&scope)?;
    }
    if matches!(
        op.as_str(),
        "device-enable" | "device-disable" | "passkey-enable" | "passkey-disable"
    ) {
        let method = if op.starts_with("passkey") {
            Method::Passkey
        } else {
            Method::Mac
        };
        return configure_device(&app, &state, &pass, method, op.ends_with("-enable"))
            .await
            .map(|()| Value::Null);
    }
    if op == "release-check" {
        return check_remote_artifact(app, &state, scope, input.platform)
            .await
            .map(|()| Value::Null);
    }
    if matches!(op.as_str(), "workflow-preview" | "workflow-create") {
        return workflow_operation(app, &state, scope, op, input, &pass)
            .await
            .map(|()| Value::Null);
    }
    if op == "apple-fetch" {
        return fetch_apple(app, &state, scope).await.map(|()| Value::Null);
    }
    let bytes = match op.as_str() {
        "import-file" | "import-text" => Some(read(&pick(&[])?, 3 * 1024 * 1024)?),
        "key-match" | "certificate" => {
            Some(read(&pick(&["cer", "der", "pem", "crt"])?, 1024 * 1024)?)
        }
        "app-connect" => Some(read(&pick(&["pem"])?, 64 * 1024)?),
        "check-conf" => Some(read(&pick(&["json"])?, 1024 * 1024)?),
        "csv-import" => Some(read(&pick(&["csv"])?, 4 * 1024 * 1024)?),
        "verify" | "manifest" | "release-add" => Some(read(&pick(&["sig"])?, 16384)?),
        _ => None,
    };
    let artifact = if matches!(op.as_str(), "verify" | "manifest" | "release-add") {
        Some(pick(&[])?)
    } else {
        None
    };
    let out = match op.as_str() {
        "backup" | "csv-export" | "manifest" | "workflow" | "release-export" => Some(
            rfd::FileDialog::new()
                .set_file_name(if op == "backup" {
                    "ReleaseVault.rvbackup"
                } else if op == "workflow" {
                    "build-updater.yml"
                } else if op == "manifest" || op == "release-export" {
                    "latest.json"
                } else {
                    "credentials.csv"
                })
                .save_file()
                .ok_or("cancel")?,
        ),
        _ => None,
    };
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Shared>();
        let k = &input.key;
        if k == "APPLE_ACCOUNT_PASSWORD"
            && matches!(op.as_str(), "github" | "remote" | "watch" | "app-inspect")
        {
            return Err("vault-only".into());
        }
        let mut w = state.lock().map_err(|_| "busy")?;
        if w.scope != scope {
            return Err("scope".into());
        }
        let v = &mut w.vault;
        let b = bytes.as_ref().map(|v| v.as_slice()).unwrap_or_default();
        let mut result = Value::Null;
        if [
            "save",
            "import-file",
            "import-text",
            "key-match",
            "certificate",
            "apple-apply",
        ]
        .contains(&op.as_str())
        {
            let doc = &v.unlocked()?.doc;
            doc.require_owner(&scope, k)?;
            let affected: Vec<_> = doc
                .dependents(&scope, k)
                .iter()
                .map(ToString::to_string)
                .collect();
            if affected != input.affected {
                return Err("impact".into());
            }
            if !affected.is_empty() {
                v.reauth(&pass)?;
            }
        }
        match op.as_str() {
            "apple-apply" => v.change(|d, key| {
                apple_api::apply(
                    d,
                    &scope,
                    &input.key,
                    &input.report_id,
                    &input.resource_id,
                    &input.resource_kind,
                    key,
                )
            })?,
            "actions-fetch" => {
                let session = v.unlocked()?;
                let report = if let Some(c) = session.doc.github_apps.get(&scope.project) {
                    c.check_installation(&input.installation_id)?;
                    github_app::runs(
                        c,
                        &input.repo,
                        input.repo_id.parse().map_err(|_| "app-config")?,
                        &session.key,
                    )?
                } else {
                    integrations::workflow_runs(&input.repo)?
                };
                v.change(|d, _| {
                    d.action_reports.insert(scope.project, report);
                    d.event(&scope, &input.repo, "actions-fetch");
                    Ok(())
                })?;
            }
            "actions-open" => {
                let session = v.unlocked()?;
                let report = session
                    .doc
                    .action_reports
                    .get(&scope.project)
                    .ok_or("missing")?;
                if report.repo != input.repo {
                    return Err("scope".into());
                }
                let id: u64 = input.run_id.parse().map_err(|_| "invalid")?;
                if !report.runs.iter().any(|run| run.id == id) {
                    return Err("missing".into());
                }
                integrations::dest(&report.repo, "")?;
                open::that(format!(
                    "https://github.com/{}/actions/runs/{id}",
                    report.repo
                ))
                .map_err(|_| "file")?;
            }
            "share" | "unlink" => {
                v.reauth(&pass)?;
                v.change(|d, _| {
                    if op == "share" {
                        d.link(&scope, k, input.owner.parse().map_err(|_| "share")?)
                    } else {
                        d.unlink(&scope, k)
                    }
                })?
            }
            "key-match" => v.change(|d, key| {
                certificates::match_certificate(d, &scope, k, b, &key_password, key)
            })?,

            "edit-project" => {
                v.change(|d, _| {
                    d.edit_project(scope.project, &input.name, input.targets.clone())?;
                    d.set_development_directory(scope.project, &val)
                })?;
                if !input.targets.contains(&scope.target) {
                    w.scope.target = input.targets[0].clone();
                }
                w.selected = String::new();
            }
            "project" => {
                let id = v.change(|d, _| {
                    let id = d.add_project(&input.name, input.targets.clone())?;
                    d.set_development_directory(id, &val)?;
                    Ok(id)
                })?;
                w.scope = Scope {
                    project: id,
                    target: input.targets[0].clone(),
                    env: scope.env,
                };
                w.selected = String::new();
            }
            "custom" => v.change(|d, _| d.custom(&scope, k, &input.name, &input.kind))?,
            "save" => v.change(|d, key| {
                if let Some(label) = &input.app_password_label {
                    if k != "APPLE_PASSWORD" {
                        return Err("invalid".into());
                    }
                    d.save_app_password(&scope, val.as_bytes(), label, &input.expires, key)
                } else {
                    d.save(&scope, k, val.as_bytes(), &input.expires, key)
                }
            })?,
            "import-file" | "import-text" => v.change(|d, key| {
                if op == "import-text" {
                    std::str::from_utf8(b).map_err(|_| "format")?;
                    d.save(&scope, k, b, &input.expires, key)
                } else {
                    d.save(
                        &scope,
                        k,
                        Zeroizing::new(STANDARD.encode(b)).as_bytes(),
                        &input.expires,
                        key,
                    )
                }
            })?,
            "clear" => {
                v.reauth(&pass)?;
                v.change(|d, _| d.clear_value(&scope, k))?
            }
            "reveal" => {
                v.reauth(&pass)?;
                result = v.change(|d, key| {
                    let b = integrations::value(d, &scope, k, key)?;
                    if b.len() > 65536 {
                        return Err("size".into());
                    }
                    let txt = std::str::from_utf8(&b).map_err(|_| "format")?;
                    d.event(&scope, k, "reveal");
                    Ok(json!(txt))
                })?
            }
            "app-connect" => {
                v.reauth(&pass)?;
                v.change(|d, key| {
                    github_app::connect(
                        d,
                        &scope,
                        key,
                        input.app_id.parse().map_err(|_| "app-config")?,
                        input.installation_id.parse().map_err(|_| "app-config")?,
                        b,
                    )
                })?;
            }
            "app-refresh" | "app-permission" | "app-disconnect" | "app-settings" => {
                let c = v
                    .unlocked()?
                    .doc
                    .github_apps
                    .get(&scope.project)
                    .ok_or("app-config")?;
                c.check_installation(&input.installation_id)?;
                match op.as_str() {
                    "app-settings" => open::that(c.settings()).map_err(|_| "file")?,
                    "app-refresh" => v.change(|d, key| github_app::refresh(d, &scope, key))?,
                    _ => {
                        v.reauth(&pass)?;
                        v.change(|d, _| {
                            if op == "app-disconnect" {
                                return github_app::disconnect(d, &scope);
                            }
                            let c = d.github_apps.get_mut(&scope.project).ok_or("app-config")?;
                            let id = input.repo_id.parse().map_err(|_| "app-config")?;
                            c.configure(id, input.grants)?;
                            d.event(&scope, &input.repo_id, "app-permission");
                            Ok(())
                        })?;
                    }
                }
            }
            "app-inspect" => v.change(|d, key| {
                let c = d.github_apps.get(&scope.project).ok_or("app-config")?;
                c.check_installation(&input.installation_id)?;
                let binding = c.binding(
                    input.repo,
                    input.environment,
                    input.repo_id.parse().map_err(|_| "app-config")?,
                );
                github_app::bind_for_read(d, &scope, k, key, binding)
            })?,
            "github" => {
                v.reauth(&pass)?;
                v.change(|d, key| {
                    if input.provider == "app" {
                        d.github_apps
                            .get(&scope.project)
                            .ok_or("app-config")?
                            .check_installation(&input.installation_id)?;
                        return github_app::send(
                            d,
                            &scope,
                            k,
                            &input.repo,
                            &input.environment,
                            input.repo_id.parse().map_err(|_| "app-config")?,
                            key,
                        );
                    }
                    if input.provider != "cli" {
                        return Err("app-config".into());
                    }
                    integrations::github(d, &scope, k, &input.repo, &input.environment, key)
                })?;
            }
            "remote" => v.change(|d, key| integrations::fetch_remote(d, &scope, k, key))?,
            "watch" => v.change(|d, _| {
                d.entry(&scope, k)?;
                let binding = d
                    .entries
                    .get_mut(&Document::address(&scope, k))
                    .and_then(|e| e.binding.as_mut())
                    .ok_or("missing")?;
                binding.automatic = !binding.automatic;
                Ok(())
            })?,
            "generate" => {
                v.reauth(&pass)?;
                v.change(|d, key| integrations::generate(d, &scope, key))?
            }
            "check-conf" => v.change(|d, key| integrations::check_conf(d, &scope, b, key))?,
            "workflow-open" => {
                let session = v.unlocked()?;
                let preview = session
                    .doc
                    .workflow_previews
                    .get(&publishing::ReleaseDraft::address(&scope))
                    .ok_or("missing")?;
                integrations::dest(&preview.repo, "")?;
                let number = preview.pr.ok_or("missing")?;
                open::that(format!("https://github.com/{}/pull/{number}", preview.repo))
                    .map_err(|_| "file")?;
            }
            "workflow" => {
                let session = v.unlocked()?;
                let yaml = if input.targets.is_empty() {
                    publishing::workflow(&scope)?
                } else {
                    publishing::project_workflow(&session.doc, &scope, &input.targets)?
                };
                save(out.as_ref().ok_or("file")?, yaml.as_bytes())?;
            }
            "release-add" => v.change(|d, key| {
                publishing::add_platform(
                    d,
                    &scope,
                    key,
                    artifact.as_ref().ok_or("file")?,
                    b,
                    publishing::ReleaseInfo {
                        version: &input.version,
                        platform: &input.platform,
                        url: &input.url,
                    },
                )
            })?,
            "release-remove" => {
                v.change(|d, _| publishing::remove_platform(d, &scope, &input.platform))?
            }
            "release-export" => {
                let session = v.unlocked()?;
                let bytes = publishing::combined_manifest(&session.doc, &scope, &session.key)?;
                save(out.as_ref().ok_or("file")?, &bytes)?;
            }
            "manifest" => {
                let s = v.unlocked()?;
                let bytes = publishing::manifest(
                    &s.doc,
                    &scope,
                    &s.key,
                    artifact.as_ref().ok_or("file")?,
                    b,
                    publishing::ReleaseInfo {
                        version: &input.version,
                        platform: &input.platform,
                        url: &input.url,
                    },
                )?;
                save(out.as_ref().ok_or("file")?, &bytes)?;
                v.change(|d, _| {
                    d.event(&scope, integrations::PUBLIC, "manifest");
                    Ok(())
                })?;
            }
            "verify" => v.change(|d, key| {
                integrations::verify_artifact(d, &scope, artifact.as_ref().ok_or("file")?, b, key)?;
                d.event(&scope, integrations::PUBLIC, "verify");
                Ok(())
            })?,
            "certificate" => v.change(|d, _| {
                d.entry(&scope, k)?;
                let (date, meta) = integrations::certificate(b)?;
                let entry = d
                    .entries
                    .get_mut(&Document::address(&scope, k))
                    .ok_or("missing")?;
                entry.expires = date;
                entry.evidence = meta;
                entry.matched_fingerprint.clear();
                d.event(&scope, k, "certificate");
                Ok(())
            })?,
            "csv-import" => {
                result = json!(v.change(|d, key| integrations::csv_import(d, &scope, b, key))?)
            }
            "csv-export" => {
                let b = integrations::csv_export(&v.unlocked()?.doc, &scope)?;
                save(out.as_ref().unwrap(), &b)?;
            }
            "backup" => {
                let b = v.backup(&pass)?;
                save(out.as_ref().unwrap(), &b)?;
            }
            "source" | "acquire" => {
                let def = v.unlocked()?.doc.def(&scope, k)?;
                open::that(guide_url(def, op == "acquire")?).map_err(|_| "file")?;
            }
            _ => return Err("invalid".into()),
        }
        drop(w);
        notify(&app);
        Ok(result)
    })
    .await
    .map_err(|_| "busy")?
}
// Resolve the selected item against the bundled catalog. Webviews supply only the action.
fn guide_url(def: &model::Def, acquisition: bool) -> R<&'static str> {
    static SOURCES: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    let sources = SOURCES.get_or_init(|| {
        serde_json::from_str(include_str!("../../resources/sources.json"))
            .expect("valid bundled guide links")
    });
    if def.guide == "custom" {
        return Err("missing".into());
    }
    let group = sources["items"][&def.key]["links"]
        .as_str()
        .ok_or("missing")?;
    sources["links"][group][if acquisition { "get" } else { "docs" }]
        .as_str()
        .ok_or_else(|| "missing".into())
}

async fn fetch_apple(app: tauri::AppHandle, state: &Shared, scope: Scope) -> R<()> {
    let (epoch, auth) = {
        let mut w = state.lock().map_err(|_| "busy")?;
        w.auth_epoch += 1;
        let epoch = w.auth_epoch;
        let session = w.vault.unlocked()?;
        (
            epoch,
            apple_api::Credentials::load(&session.doc, &scope, &session.key)?,
        )
    };
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Shared>();
        let validate = || -> R<()> {
            let mut w = state.lock().map_err(|_| "busy")?;
            if w.auth_epoch != epoch || w.scope != scope {
                return Err("cancel".into());
            }
            let session = w.vault.unlocked()?;
            if apple_api::Credentials::load(&session.doc, &scope, &session.key)?.fingerprint()
                != auth.fingerprint()
            {
                return Err("apple-stale".into());
            }
            Ok(())
        };
        let report = apple_api::fetch(&apple_api::Api::apple(), &auth, validate)?;
        let mut w = state.lock().map_err(|_| "busy")?;
        if w.auth_epoch != epoch || w.scope != scope {
            return Err("cancel".into());
        }
        w.vault.change(|d, key| {
            if apple_api::Credentials::load(d, &scope, key)?.fingerprint() != report.credentials {
                return Err("apple-stale".into());
            }
            d.apple_reports
                .insert(apple_api::Report::address(&scope), report);
            d.event(&scope, "", "apple-fetch");
            Ok(())
        })?;
        drop(w);
        notify(&app);
        Ok(())
    })
    .await
    .map_err(|_| "apple-network")?
}

async fn workflow_operation(
    app: tauri::AppHandle,
    state: &Shared,
    scope: Scope,
    op: String,
    input: Input,
    password: &str,
) -> R<()> {
    let writing = op == "workflow-create";
    let (epoch, doc, key) = {
        let mut w = state.lock().map_err(|_| "busy")?;
        if writing {
            w.vault.reauth(password)?;
        }
        w.auth_epoch += 1;
        let epoch = w.auth_epoch;
        let session = w.vault.unlocked()?;
        (epoch, session.doc.clone(), session.key.clone())
    };
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Shared>();
        let connection = doc.github_apps.get(&scope.project);
        let settings = serde_json::to_vec(&connection).map_err(|_| "format")?;
        let validate = || -> R<()> {
            let mut w = state.lock().map_err(|_| "busy")?;
            if w.auth_epoch != epoch || w.scope != scope {
                return Err("workflow-stale".into());
            }
            let current = w.vault.unlocked()?;
            if serde_json::to_vec(&current.doc.github_apps.get(&scope.project))
                .map_err(|_| "format")?
                != settings
            {
                return Err("workflow-stale".into());
            }
            if writing
                && current
                    .doc
                    .workflow_previews
                    .get(&publishing::ReleaseDraft::address(&scope))
                    .is_none_or(|p| p.id != input.preview_id)
            {
                return Err("workflow-stale".into());
            }
            Ok(())
        };
        let saved = if writing {
            Some(
                doc.workflow_previews
                    .get(&publishing::ReleaseDraft::address(&scope))
                    .filter(|p| p.id == input.preview_id)
                    .ok_or("workflow-stale")?
                    .clone(),
            )
        } else {
            None
        };
        let (repo, id, installation) = if let Some(preview) = &saved {
            (
                preview.repo.clone(),
                preview.repository,
                preview.installation,
            )
        } else {
            (
                input.repo.clone(),
                input.repo_id.parse().unwrap_or(0),
                connection
                    .map(|_| input.installation_id.parse())
                    .transpose()
                    .map_err(|_| "app-config")?,
            )
        };
        validate()?;
        let client: Box<dyn workflow_pr::Transport> = match (connection, installation) {
            (Some(c), Some(installation)) => {
                c.check_installation(&installation.to_string())?;
                Box::new(github_app::workflow_client(c, &repo, id, &key, writing)?)
            }
            (None, None) => Box::new(integrations::CliWorkflowClient),
            _ => return Err("workflow-stale".into()),
        };
        let mut client = workflow_pr::GuardedClient {
            client,
            validate: &validate,
        };
        let preview = if let Some(mut preview) = saved {
            preview.pr = Some(workflow_pr::create(&mut client, &preview)?);
            preview
        } else {
            let yaml = publishing::project_workflow(&doc, &scope, &input.targets)?;
            workflow_pr::preview(&mut client, &repo, id, installation, yaml)?
        };
        validate()?;
        let mut w = state.lock().map_err(|_| "busy")?;
        if w.auth_epoch != epoch || w.scope != scope {
            return Err("workflow-stale".into());
        }
        w.vault.change(|d, _| {
            d.workflow_previews
                .insert(publishing::ReleaseDraft::address(&scope), preview);
            if writing {
                d.event(&scope, &repo, "workflow-create");
            }
            Ok(())
        })?;
        drop(w);
        notify(&app);
        Ok(())
    })
    .await
    .map_err(|_| "workflow-network")?
}

async fn check_remote_artifact(
    app: tauri::AppHandle,
    state: &Shared,
    scope: Scope,
    platform: String,
) -> R<()> {
    let (epoch, version, entry) = {
        let mut w = state.lock().map_err(|_| "busy")?;
        let epoch = w.auth_epoch;
        let session = w.vault.unlocked()?;
        let draft = session
            .doc
            .release_drafts
            .get(&publishing::ReleaseDraft::address(&scope))
            .ok_or("missing")?;
        let entry = draft.platforms.get(&platform).ok_or("missing")?.clone();
        let target = Scope {
            target: entry.target.clone(),
            ..scope.clone()
        };
        let public =
            integrations::value(&session.doc, &target, integrations::PUBLIC, &session.key)?;
        if public.as_slice() != entry.public_key {
            return Err("release-key".into());
        }
        (epoch, draft.version.clone(), entry)
    };
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Shared>();
        let cancelled = || {
            let Ok(mut w) = state.lock() else { return true };
            w.vault.expire();
            w.auth_epoch != epoch || w.scope != scope || w.vault.session.is_none()
        };
        let result = remote_artifact::verify(&entry, &version, cancelled);
        let mut w = state.lock().map_err(|_| "busy")?;
        if w.auth_epoch != epoch || w.scope != scope {
            return Err("cancel".into());
        }
        w.vault.change(|d, key| {
            publishing::record_remote_check(d, &scope, key, &platform, &version, &entry, result)?;
            d.event(&scope, &platform, "release-check");
            Ok(())
        })?;
        drop(w);
        notify(&app);
        Ok(())
    })
    .await
    .map_err(|_| "download")?
}

async fn configure_device(
    app: &tauri::AppHandle,
    state: &Shared,
    password: &str,
    method: Method,
    enable: bool,
) -> R<()> {
    let (epoch, key) = {
        let mut w = state.lock().map_err(|_| "busy")?;
        w.vault.reauth(password)?;
        w.auth_epoch += 1;
        if !enable {
            w.vault.set_method_record(method, None)?;
            drop(w);
            notify(app);
            return Ok(());
        }
        (w.auth_epoch, w.vault.unlocked()?.key.clone())
    };
    let record = tauri::async_runtime::spawn_blocking(move || {
        let record = method.enroll(&key)?;
        if method == Method::Mac && *method.open(&record)? != *key {
            return Err("device".to_owned());
        }
        Ok(record)
    })
    .await
    .map_err(|_| method.error())??;
    let mut w = state.lock().map_err(|_| "busy")?;
    if w.auth_epoch != epoch {
        return Err("locked".into());
    }
    w.vault.set_method_record(method, Some(&record))?;
    drop(w);
    notify(app);
    Ok(())
}
// Both windows are top-level: no parent or owner is assigned to the inspector.
fn open_workspace_window(
    app: &tauri::AppHandle,
    label: &str,
) -> tauri::Result<tauri::WebviewWindow> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|config| config.label == label)
        .expect("workspace window must be configured");
    tauri::WebviewWindowBuilder::from_config(app, config)?.build()
}

#[tauri::command]
async fn float_panel(app: tauri::AppHandle, state: State<'_, Shared>, floating: bool) -> R<()> {
    state.lock().map_err(|_| "busy")?.vault.unlocked()?;
    if floating {
        if let Some(window) = app.get_webview_window("inspector") {
            window.set_focus().map_err(|_| "window")?;
        } else {
            open_workspace_window(&app, "inspector").map_err(|_| "window")?;
        }
    } else if let Some(window) = app.get_webview_window("inspector") {
        window.close().map_err(|_| "window")?;
    }
    notify(&app);
    Ok(())
}
#[tauri::command]
async fn open_preferences(app: tauri::AppHandle) -> R<()> {
    if let Some(window) = app.get_webview_window("preferences") {
        window.unminimize().map_err(|_| "window")?;
        window.set_focus().map_err(|_| "window")?;
    } else {
        open_workspace_window(&app, "preferences").map_err(|_| "window")?;
    }
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let mut args = std::env::args_os().skip(1);
            let mut dir = app.path().app_data_dir()?;
            while let Some(a) = args.next() {
                if a == "--vault-dir" {
                    dir = PathBuf::from(
                        args.next()
                            .ok_or_else(|| std::io::Error::other("Missing vault directory"))?,
                    );
                    if !dir.is_absolute() {
                        return Err(std::io::Error::other("Use an absolute vault directory").into());
                    }
                }
            }
            std::fs::create_dir_all(&dir)?;
            app.manage(Mutex::new(Workspace {
                vault: Vault::open(&dir.join("vault.sqlite")).map_err(std::io::Error::other)?,
                scope: Scope::default(),
                selected: String::new(),
                auth_epoch: 0,
            }));
            open_workspace_window(app.handle(), "main")?;
            let poll = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(60));
                let state = poll.state::<Shared>();
                if let Ok(mut w) = state.try_lock() {
                    if w.vault.refresh_due().unwrap_or(false) {
                        notify(&poll)
                    }
                };
            });
            let h = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(2));
                let state = h.state::<Shared>();
                if let Ok(mut w) = state.try_lock() {
                    let was = w.vault.session.is_some();
                    w.vault.expire();
                    if was && w.vault.session.is_none() {
                        notify(&h)
                    }
                };
            });
            Ok(())
        })
        .on_window_event(|w, e| {
            if matches!(e, tauri::WindowEvent::Destroyed) {
                if w.label() == "main" {
                    w.app_handle().exit(0)
                } else {
                    notify(w.app_handle())
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            authenticate,
            lock,
            touch,
            snapshot,
            select,
            perform,
            float_panel,
            open_preferences,
            cancel_remote_check
        ])
        .run(tauri::generate_context!())
        .expect("ReleaseVault runtime");
}
#[cfg(test)]
mod tests;
