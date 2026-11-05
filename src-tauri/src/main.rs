#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod integrations;
mod model;
mod publishing;
mod vault;
use base64::{engine::general_purpose::STANDARD, Engine};
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
}
type Shared = Mutex<Workspace>;
fn notify(app: &tauri::AppHandle) {
    let _ = app.emit("workspace-changed", ());
}
fn panel(app: &tauri::AppHandle) -> bool {
    app.get_webview_window("inspector").is_some()
}
#[tauri::command]
fn snapshot(app: tauri::AppHandle, state: State<Shared>) -> R<Value> {
    let mut w = state.lock().map_err(|_| "busy")?;
    w.vault.expire();
    let Some(s) = &w.vault.session else {
        return Ok(json!({"locked":true,"exists":w.vault.exists()?}));
    };
    let d = &s.doc;
    let rows=d.defs(&w.scope)?.into_iter().map(|f|{
 let e=d.entry(&w.scope,&f.key).ok();let display=if f.kind!="secret"{e.and_then(|x|vault::unseal(&s.key,&x.cipher).ok()).and_then(|x|String::from_utf8(x.to_vec()).ok()).map(|v|v.chars().take(2048).collect::<String>())}else{None};
 json!({"def":f,"present":e.is_some(),"display":display,"expires":e.map(|x|&x.expires),"synced":e.map(|x|&x.synced),"binding":e.and_then(|x|x.binding.as_ref()),"evidence":e.map(|x|&x.evidence)})
 }).collect::<Vec<_>>();
    Ok(
        json!({"locked":false,"scope":w.scope,"projects":d.projects,"rows":rows,"selected":w.selected,"floating":panel(&app),"audit":d.audit.iter().rev().filter(|e|e.project==w.scope.project&&e.env==w.scope.env).take(150).collect::<Vec<_>>()}),
    )
}
#[tauri::command]
fn touch(state: State<Shared>) -> R<()> {
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
    let mut w = state.lock().map_err(|_| "busy")?;
    match mode.as_str() {
        "create" => w.vault.create(&p)?,
        "unlock" => w.vault.unlock(&p)?,
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
fn lock(app: tauri::AppHandle, state: State<Shared>) -> R<()> {
    state.lock().map_err(|_| "busy")?.vault.session = None;
    notify(&app);
    Ok(())
}
#[tauri::command]
fn select(app: tauri::AppHandle, state: State<Shared>, scope: Scope, key: String) -> R<()> {
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
    name: String,
    kind: String,
    value: String,
    expires: String,
    password: String,
    repo: String,
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
    let pass = Zeroizing::new(std::mem::take(&mut input.password));
    let val = Zeroizing::new(std::mem::take(&mut input.value));
    let k = &input.key;
    {
        let mut w = state.lock().map_err(|_| "busy")?;
        if w.scope != scope {
            return Err("scope".into());
        }
        w.vault.unlocked()?.doc.project(&scope)?;
    }
    let bytes = match op.as_str() {
        "import-file" | "import-text" => Some(read(&pick(&[])?, 3 * 1024 * 1024)?),
        "certificate" => Some(read(&pick(&["cer", "der", "pem", "crt"])?, 1024 * 1024)?),
        "check-conf" => Some(read(&pick(&["json"])?, 1024 * 1024)?),
        "csv-import" => Some(read(&pick(&["csv"])?, 4 * 1024 * 1024)?),
        "verify" | "manifest" => Some(read(&pick(&["sig"])?, 16384)?),
        _ => None,
    };
    let artifact = if op == "verify" || op == "manifest" {
        Some(pick(&[])?)
    } else {
        None
    };
    let out = match op.as_str() {
        "backup" | "csv-export" | "manifest" | "workflow" => Some(
            rfd::FileDialog::new()
                .set_file_name(if op == "backup" {
                    "ReleaseVault.rvbackup"
                } else if op == "workflow" {
                    "build-updater.yml"
                } else if op == "manifest" {
                    "latest.json"
                } else {
                    "credentials.csv"
                })
                .save_file()
                .ok_or("cancel")?,
        ),
        _ => None,
    };
    let mut w = state.lock().map_err(|_| "busy")?;
    if w.scope != scope {
        return Err("scope".into());
    }
    let v = &mut w.vault;
    let b = bytes.as_ref().map(|v| v.as_slice()).unwrap_or_default();
    let mut result = Value::Null;
    match op.as_str() {
        "project" => {
            let id = v.change(|d, _| d.add_project(&input.name, input.targets.clone()))?;
            w.scope = Scope {
                project: id,
                target: input.targets[0].clone(),
                env: scope.env,
            };
            w.selected = String::new();
        }
        "custom" => v.change(|d, _| d.custom(&scope, k, &input.name, &input.kind))?,
        "save" => v.change(|d, key| d.save(&scope, k, val.as_bytes(), &input.expires, key))?,
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
            v.change(|d, _| {
                d.def(&scope, k)?;
                d.entries.remove(&Document::address(&scope, k));
                d.event(&scope, k, "clear");
                Ok(())
            })?
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
        "github" => {
            v.reauth(&pass)?;
            v.change(|d, key| {
                integrations::github(d, &scope, k, &input.repo, &input.environment, key)
            })?;
        }
        "remote" => v.change(|d, _| integrations::fetch_remote(d, &scope, k))?,
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
        "workflow" => {
            v.unlocked()?;
            save(
                out.as_ref().ok_or("file")?,
                publishing::workflow(&scope)?.as_bytes(),
            )?;
        }
        "manifest" => {
            let s = v.unlocked()?;
            let bytes = publishing::manifest(
                &s.doc,
                &scope,
                &s.key,
                artifact.as_ref().ok_or("file")?,
                b,
                &input.version,
                &input.platform,
                &input.url,
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
        "source" => {
            let guide = &v.unlocked()?.doc.def(&scope, k)?.guide;
            let urls: std::collections::HashMap<String, String> =
                serde_json::from_str(include_str!("../../resources/sources.json"))
                    .map_err(|_| "format")?;
            open::that(urls.get(guide).ok_or("missing")?).map_err(|_| "file")?;
        }
        _ => return Err("invalid".into()),
    }
    drop(w);
    notify(&app);
    Ok(result)
}
#[tauri::command]
async fn float_panel(app: tauri::AppHandle, state: State<'_, Shared>, floating: bool) -> R<()> {
    state.lock().map_err(|_| "busy")?.vault.unlocked()?;
    if floating && !panel(&app) {
        tauri::WebviewWindowBuilder::new(
            &app,
            "inspector",
            tauri::WebviewUrl::App("index.html?panel=inspector".into()),
        )
        .title("ReleaseVault · Inspector")
        .inner_size(370., 660.)
        .min_inner_size(300., 420.)
        .build()
        .map_err(|_| "window")?;
    } else if !floating {
        if let Some(w) = app.get_webview_window("inspector") {
            w.close().map_err(|_| "window")?;
        }
    }
    notify(&app);
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
            }));
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
            float_panel
        ])
        .run(tauri::generate_context!())
        .expect("ReleaseVault runtime");
}
#[cfg(test)]
mod tests;
