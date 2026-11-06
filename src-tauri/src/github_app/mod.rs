mod api;
#[cfg(test)]
mod tests;
use crate::{
    integrations,
    model::{key_ok, Binding, Document, Scope},
    vault::{self, Key, R},
};
use api::{jwt, Api};
use base64::{engine::general_purpose::STANDARD, Engine};
use crypto_box::PublicKey;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    #[default]
    None,
    Read,
    Write,
}
impl Level {
    fn wire(self) -> &'static str {
        match self {
            Self::Write => "write",
            Self::Read => "read",
            Self::None => "none",
        }
    }
}
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Grants {
    #[serde(default)]
    pub secrets: Level,
    #[serde(default)]
    pub actions: Level,
    #[serde(default)]
    pub contents: Level,
    #[serde(default)]
    pub workflows: Level,
    #[serde(default)]
    pub pull_requests: Level,
    #[serde(default, alias = "actions_variables")]
    pub variables: Level,
    #[serde(default)]
    pub environments: Level,
}
impl Grants {
    fn intersect(&self, other: &Self) -> Self {
        Self {
            secrets: self.secrets.min(other.secrets),
            actions: self.actions.min(other.actions),
            contents: self.contents.min(other.contents),
            workflows: self.workflows.min(other.workflows),
            pull_requests: self.pull_requests.min(other.pull_requests),
            variables: self.variables.min(other.variables),
            environments: self.environments.min(other.environments),
        }
    }
    fn allows(&self, p: &str, need: Level) -> bool {
        (match p {
            "secrets" => self.secrets,
            "actions" => self.actions,
            "contents" => self.contents,
            "workflows" => self.workflows,
            "pull_requests" => self.pull_requests,
            "actions_variables" => self.variables,
            "environments" => self.environments,
            _ => Level::None,
        }) >= need
    }
    fn supported(&self, limit: &Self) -> bool {
        self.contents <= limit.contents
            && self.workflows <= limit.workflows
            && self.pull_requests <= limit.pull_requests
            && self.actions <= limit.actions
            && self.secrets <= limit.secrets
            && self.variables <= limit.variables
            && self.environments <= limit.environments
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Repo {
    pub id: u64,
    pub full_name: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Connection {
    app: u64,
    installation: u64,
    #[serde(with = "vault::blob")]
    pem: Vec<u8>,
    account: String,
    organization: bool,
    limits: Grants,
    repos: Vec<Repo>,
    policy: BTreeMap<u64, Grants>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct RemoteId {
    pub installation: u64,
    pub repository: u64,
}

impl Connection {
    pub fn view(&self) -> Value {
        json!({"app":self.app.to_string(),"installation":self.installation.to_string(),"account":self.account,"limits":self.limits,
        "repos":self.repos.iter().map(|r|json!({"id":r.id.to_string(),"name":r.full_name,"grants":self.policy.get(&r.id).cloned().unwrap_or_default()})).collect::<Vec<_>>()})
    }
    pub fn check_installation(&self, id: &str) -> R<()> {
        if id == self.installation.to_string() {
            Ok(())
        } else {
            Err("app-config".into())
        }
    }
    pub fn binding(&self, repo: String, environment: String, id: u64) -> Binding {
        Binding {
            app: Some(RemoteId {
                installation: self.installation,
                repository: id,
            }),
            repo,
            environment,
            ..Default::default()
        }
    }
    pub fn settings(&self) -> String {
        if self.organization {
            format!(
                "https://github.com/organizations/{}/settings/installations/{}",
                self.account, self.installation
            )
        } else {
            format!(
                "https://github.com/settings/installations/{}",
                self.installation
            )
        }
    }
    fn permission(&self, id: u64, name: &str, p: &str, level: Level) -> R<()> {
        let valid_repo = self.repos.iter().any(|r| r.id == id && r.full_name == name);
        if !valid_repo
            || !self.limits.allows(p, level)
            || !self.policy.get(&id).is_some_and(|g| g.allows(p, level))
        {
            return Err("app-policy".into());
        }
        Ok(())
    }
    pub fn configure(&mut self, id: u64, grants: Grants) -> R<()> {
        if !self.repos.iter().any(|r| r.id == id) || !grants.supported(&self.limits) {
            return Err("app-policy".into());
        }
        self.policy.insert(id, grants);
        Ok(())
    }
}

fn validate_repo(full_name: &str) -> R<()> {
    let (owner, repo) = full_name.split_once('/').ok_or("destination")?;
    if owner.is_empty()
        || !owner
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        || repo.is_empty()
        || repo == "."
        || repo == ".."
        || !repo
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err("destination".into());
    }
    Ok(())
}
fn discover(api: &Api, c: &mut Connection, pem: &[u8]) -> R<()> {
    let signed = jwt(c.app, pem)?;
    let info = api
        .request(
            "GET",
            &format!("/app/installations/{}", c.installation),
            &signed,
            None,
        )?
        .success()?;
    if info["id"].as_u64() != Some(c.installation)
        || info["app_id"].as_u64() != Some(c.app)
        || !info["suspended_at"].is_null()
    {
        return Err("app-auth".into());
    }
    let login = info["account"]["login"].as_str().ok_or("app-api")?;
    validate_repo(&format!("{login}/example"))?;
    c.organization = match info["target_type"].as_str() {
        Some("Organization") => true,
        Some("User") => false,
        _ => return Err("app-grant".into()),
    };
    c.account = login.into();
    c.limits = serde_json::from_value(info["permissions"].clone()).map_err(|_| "app-api")?;
    let token = api.token(&signed, c.installation, None, "metadata", "read")?;
    let mut repos = Vec::new();
    let start = Instant::now();
    for page in 1.. {
        if start.elapsed() > Duration::from_secs(60) {
            return Err("app-network".into());
        }
        let data = api
            .request(
                "GET",
                &format!("/installation/repositories?per_page=100&page={page}"),
                &token,
                None,
            )?
            .success()?;
        let batch: Vec<Repo> =
            serde_json::from_value(data["repositories"].clone()).map_err(|_| "app-api")?;
        let last = batch.len() < 100;
        for repo in batch {
            validate_repo(&repo.full_name)?;
            repos.push(repo);
        }
        if last {
            break;
        }
    }
    repos.sort_by_key(|r| r.full_name.to_lowercase());
    c.policy.retain(|id, g| {
        *g = g.intersect(&c.limits);
        repos.iter().any(|r| r.id == *id)
    });
    c.repos = repos;
    Ok(())
}
pub fn connect(
    d: &mut Document,
    s: &Scope,
    key: &Key,
    app: u64,
    installation: u64,
    pem: &[u8],
) -> R<()> {
    if app == 0 || installation == 0 || d.github_apps.contains_key(&s.project) {
        return Err("app-config".into());
    }
    let mut c = Connection {
        app,
        installation,
        pem: vault::seal(key, pem)?,
        account: String::new(),
        organization: false,
        limits: Grants::default(),
        repos: vec![],
        policy: BTreeMap::new(),
    };
    discover(&Api::github(), &mut c, pem)?;
    stop_watching(d, s);
    d.github_apps.insert(s.project, c);
    d.event(s, "", "app-connect");
    Ok(())
}
pub fn refresh(d: &mut Document, s: &Scope, key: &Key) -> R<()> {
    let c = d.github_apps.get_mut(&s.project).ok_or("app-config")?;
    let pem = vault::unseal(key, &c.pem)?;
    discover(&Api::github(), c, &pem)?;
    d.event(s, "", "app-refresh");
    Ok(())
}
pub fn disconnect(d: &mut Document, s: &Scope) -> R<()> {
    d.github_apps.remove(&s.project).ok_or("app-config")?;
    stop_watching(d, s);
    d.event(s, "", "app-disconnect");
    Ok(())
}
fn stop_watching(d: &mut Document, s: &Scope) {
    let prefix = format!("{}/", s.project);
    for (_, entry) in d
        .entries
        .range_mut(prefix.clone()..)
        .take_while(|(k, _)| k.starts_with(&prefix))
    {
        if let Some(b) = &mut entry.binding {
            b.automatic = false;
        }
    }
}
struct Destination<'a> {
    id: u64,
    name: &'a str,
    env: &'a str,
    secret: bool,
}
impl Destination<'_> {
    fn permission(&self) -> &'static str {
        if !self.env.is_empty() {
            "environments"
        } else if self.secret {
            "secrets"
        } else {
            "actions_variables"
        }
    }
    fn path(&self) -> R<String> {
        validate_repo(self.name)?;
        let base = if self.env.is_empty() {
            format!("/repos/{}/actions", self.name)
        } else {
            if self.env.len() > 255
                || self.env == "."
                || self.env == ".."
                || self.env.chars().any(char::is_control)
            {
                return Err("destination".into());
            }
            let mut url = tauri::Url::parse("https://github.com").map_err(|_| "destination")?;
            url.path_segments_mut()
                .map_err(|_| "destination")?
                .push(self.env);
            let encoded = url.path().trim_start_matches('/');
            format!("/repos/{}/environments/{encoded}", self.name)
        };
        Ok(format!(
            "{base}/{}",
            if self.secret { "secrets" } else { "variables" }
        ))
    }
}
fn scoped_token(
    api: &Api,
    c: &Connection,
    dest: &Destination,
    key: &Key,
    level: Level,
) -> R<zeroize::Zeroizing<String>> {
    let perm = dest.permission();
    c.permission(dest.id, dest.name, perm, level)?;
    let pem = vault::unseal(key, &c.pem)?;
    let signed = jwt(c.app, &pem)?;
    api.token(&signed, c.installation, Some(dest.id), perm, level.wire())
}
pub fn send(
    d: &mut Document,
    s: &Scope,
    k: &str,
    repo: &str,
    env: &str,
    id: u64,
    key: &Key,
) -> R<()> {
    send_with(
        &Api::github(),
        d,
        s,
        k,
        Destination {
            id,
            name: repo,
            env,
            secret: d.def(s, k)?.kind == "secret",
        },
        key,
    )
}
fn send_with(
    api: &Api,
    d: &mut Document,
    s: &Scope,
    k: &str,
    dest: Destination,
    key: &Key,
) -> R<()> {
    if !key_ok(k) {
        return Err("reserved".into());
    }
    let c = d.github_apps.get(&s.project).ok_or("app-config")?;
    let path = dest.path()?;
    c.permission(dest.id, dest.name, dest.permission(), Level::Write)?;
    let bytes = integrations::value(d, s, k, key)?;
    if bytes.len() > 48 * 1024 {
        return Err("limit".into());
    }
    let token = scoped_token(api, c, &dest, key, Level::Write)?;
    if dest.secret {
        let pk = api
            .request("GET", &format!("{path}/public-key"), &token, None)?
            .success()?;
        let raw: [u8; 32] = STANDARD
            .decode(pk["key"].as_str().ok_or("app-api")?)
            .map_err(|_| "app-api")?
            .try_into()
            .map_err(|_| "app-api")?;
        let sealed = PublicKey::from(raw)
            .seal(&mut rand::rngs::OsRng, &bytes)
            .map_err(|_| "crypto")?;
        let key_id = pk["key_id"].as_str().ok_or("app-api")?;
        api.request(
            "PUT",
            &format!("{path}/{k}"),
            &token,
            Some(json!({"encrypted_value":STANDARD.encode(sealed),"key_id":key_id})),
        )?
        .success()?;
    } else {
        let body = json!({"name":k,"value":std::str::from_utf8(&bytes).map_err(|_|"format")?});
        let update = api.request("PATCH", &format!("{path}/{k}"), &token, Some(body.clone()))?;
        if update.status == 404 {
            api.request("POST", &path, &token, Some(body))?.success()?;
        } else {
            update.success()?;
        }
    }
    let remote = RemoteId {
        installation: c.installation,
        repository: dest.id,
    };
    integrations::record_sent(d, s, k, dest.name, dest.env, Some(remote))
}
pub fn bind_for_read(d: &mut Document, s: &Scope, k: &str, key: &Key, b: Binding) -> R<()> {
    bind_with(&Api::github(), d, s, k, key, b)
}
fn bind_with(api: &Api, d: &mut Document, s: &Scope, k: &str, key: &Key, mut b: Binding) -> R<()> {
    d.local_entry(s, k)?;
    b.remote = poll_with(api, d, s, k, key, &b)?;
    b.polled = chrono::Utc::now().to_rfc3339();
    d.entries
        .get_mut(&Document::address(s, k))
        .ok_or("missing")?
        .binding = Some(b);
    d.event(s, k, "app-inspect");
    Ok(())
}
pub fn poll(d: &Document, s: &Scope, k: &str, key: &Key, b: &Binding) -> R<String> {
    poll_with(&Api::github(), d, s, k, key, b)
}
fn poll_with(api: &Api, d: &Document, s: &Scope, k: &str, key: &Key, b: &Binding) -> R<String> {
    if !key_ok(k) {
        return Err("reserved".into());
    }
    let c = d.github_apps.get(&s.project).ok_or("app-config")?;
    let remote = b
        .app
        .as_ref()
        .filter(|r| r.installation == c.installation)
        .ok_or("app-policy")?;
    let dest = Destination {
        id: remote.repository,
        name: &b.repo,
        env: &b.environment,
        secret: d.def(s, k)?.kind == "secret",
    };
    let path = dest.path()?;
    let token = scoped_token(api, c, &dest, key, Level::Read)?;
    let response = api.request("GET", &format!("{path}/{k}"), &token, None)?;
    // A GitHub 404 can mean hidden/inaccessible; do not claim a credential is absent.
    let data = response.success()?;
    let timestamp = data["updated_at"].as_str().ok_or("app-api")?;
    chrono::DateTime::parse_from_rfc3339(timestamp).map_err(|_| "app-api")?;
    Ok(timestamp.into())
}

pub fn runs(c: &Connection, repo: &str, id: u64, key: &Key) -> R<crate::actions::Report> {
    runs_with(&Api::github(), c, repo, id, key)
}
fn runs_with(
    api: &Api,
    c: &Connection,
    repo: &str,
    id: u64,
    key: &Key,
) -> R<crate::actions::Report> {
    validate_repo(repo)?;
    c.permission(id, repo, "actions", Level::Read)?;
    let pem = vault::unseal(key, &c.pem)?;
    let signed = jwt(c.app, &pem)?;
    let token = api.token(&signed, c.installation, Some(id), "actions", "read")?;
    let result = api
        .request(
            "GET",
            &format!("/repos/{repo}/actions/runs?per_page=20"),
            &token,
            None,
        )?
        .success()?;
    crate::actions::parse(repo, result)
}

pub struct WorkflowClient {
    api: Api,
    token: zeroize::Zeroizing<String>,
}
impl crate::workflow_pr::Transport for WorkflowClient {
    fn request(&mut self, method: &str, path: &str, body: Option<Value>) -> R<(u16, Value)> {
        let reply = self.api.request(method, path, &self.token, body)?;
        Ok((reply.status, reply.data))
    }
}
pub fn workflow_client(
    c: &Connection,
    repo: &str,
    id: u64,
    key: &Key,
    write: bool,
) -> R<WorkflowClient> {
    workflow_client_with(Api::github(), c, repo, id, key, write)
}
fn workflow_client_with(
    api: Api,
    c: &Connection,
    repo: &str,
    id: u64,
    key: &Key,
    write: bool,
) -> R<WorkflowClient> {
    validate_repo(repo)?;
    let level = if write { Level::Write } else { Level::Read };
    c.permission(id, repo, "contents", level)?;
    let mut permissions = json!({"contents":level.wire()});
    if write {
        c.permission(id, repo, "workflows", Level::Write)?;
        c.permission(id, repo, "pull_requests", Level::Write)?;
        permissions["workflows"] = json!("write");
        permissions["pull_requests"] = json!("write");
    }
    let pem = vault::unseal(key, &c.pem)?;
    let signed = jwt(c.app, &pem)?;
    let mut data = api
        .request(
            "POST",
            &format!("/app/installations/{}/access_tokens", c.installation),
            &signed,
            Some(json!({"repository_ids":[id],"permissions":permissions})),
        )?
        .success()?;
    let token = match data["token"].take() {
        Value::String(token) if !token.is_empty() => zeroize::Zeroizing::new(token),
        _ => return Err("app-api".into()),
    };
    Ok(WorkflowClient { api, token })
}
