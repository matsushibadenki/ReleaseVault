use crate::vault::{self, Key, R};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Serialize, Deserialize)]
pub struct Def {
    pub key: String,
    pub name: String,
    pub kind: String,
    pub group: String,
    pub guide: String,
    pub targets: Vec<String>,
}
pub fn catalog() -> &'static Vec<Def> {
    static C: std::sync::OnceLock<Vec<Def>> = std::sync::OnceLock::new();
    C.get_or_init(|| serde_json::from_str(include_str!("../../resources/catalog.json")).unwrap())
}
pub const ENVS: [&str; 3] = ["Development", "Staging", "Production"];
#[derive(Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: u64,
    pub name: String,
    pub targets: Vec<String>,
    pub custom: Vec<Def>,
}
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Binding {
    #[serde(default)]
    pub app: Option<crate::github_app::RemoteId>,
    pub repo: String,
    pub environment: String,
    pub polled: String,
    pub remote: String,
    pub automatic: bool,
}
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Entry {
    #[serde(default)]
    pub owner: Option<u64>,
    #[serde(default)]
    pub matched_fingerprint: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub app_password_label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id_prefix: Option<crate::apple_ids::AppIdPrefix>,
    #[serde(default)]
    pub binding: Option<Binding>,
    #[serde(with = "vault::blob")]
    pub cipher: Vec<u8>,
    pub expires: String,
    pub synced: String,
    pub evidence: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Event {
    pub at: String,
    pub action: String,
    pub project: u64,
    pub env: String,
    pub key: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Document {
    #[serde(default)]
    pub github_apps: BTreeMap<u64, crate::github_app::Connection>,
    pub projects: Vec<Project>,
    pub entries: BTreeMap<String, Entry>,
    pub audit: Vec<Event>,
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Scope {
    pub project: u64,
    pub target: String,
    pub env: String,
}
impl Default for Scope {
    fn default() -> Self {
        Self {
            project: 1,
            target: "mac-direct".into(),
            env: "Production".into(),
        }
    }
}
impl Document {
    pub fn initial() -> Self {
        Self {
            projects: vec![Project {
                id: 1,
                name: "ReleaseVault".into(),
                targets: vec!["mac-direct".into(), "mac-store".into()],
                custom: vec![],
            }],
            github_apps: BTreeMap::new(),
            entries: BTreeMap::new(),
            audit: vec![],
        }
    }
    pub fn project(&self, s: &Scope) -> R<&Project> {
        self.projects
            .iter()
            .find(|p| {
                p.id == s.project && p.targets.contains(&s.target) && ENVS.contains(&s.env.as_str())
            })
            .ok_or_else(|| "scope".into())
    }
    pub fn defs(&self, s: &Scope) -> R<Vec<&Def>> {
        let p = self.project(s)?;
        Ok(catalog()
            .iter()
            .chain(&p.custom)
            .filter(|d| d.targets.contains(&s.target))
            .collect())
    }
    pub fn def(&self, s: &Scope, k: &str) -> R<&Def> {
        self.defs(s)?
            .into_iter()
            .find(|d| d.key == k)
            .ok_or_else(|| "key".into())
    }
    pub fn address(s: &Scope, k: &str) -> String {
        format!("{}/{}/{}", s.project, s.env, k)
    }
    pub fn event(&mut self, s: &Scope, k: &str, action: &str) {
        self.audit.push(Event {
            at: chrono::Utc::now().to_rfc3339(),
            action: action.into(),
            project: s.project,
            env: s.env.clone(),
            key: k.into(),
        });
    }
    pub fn save(&mut self, s: &Scope, k: &str, b: &[u8], expires: &str, key: &Key) -> R<()> {
        self.def(s, k)?;
        self.require_owner(s, k)?;
        if [
            crate::integrations::PRIVATE,
            crate::integrations::PUBLIC,
            crate::integrations::PASS,
        ]
        .contains(&k)
            && self.entry(s, k).is_ok()
        {
            return Err("occupied".into());
        }
        if b.is_empty() || b.len() > 4 * 1024 * 1024 {
            return Err("size".into());
        }
        if !expires.is_empty() {
            chrono::NaiveDate::parse_from_str(expires, "%Y-%m-%d").map_err(|_| "date")?;
        }
        let app_id_prefix = if k == crate::apple_ids::APP_ID && self.def(s, k)?.guide == "apple-id"
        {
            self.entered_app_prefix(s, b, key)?
        } else {
            None
        };
        self.entries.insert(
            Self::address(s, k),
            Entry {
                app_id_prefix,
                cipher: vault::seal(key, b)?,
                expires: expires.into(),
                ..Default::default()
            },
        );
        self.invalidate_linked_metadata(s, k);
        self.event(s, k, "save");
        Ok(())
    }
    pub(crate) fn invalidate_linked_metadata(&mut self, s: &Scope, k: &str) {
        for project in self.dependents(s, k) {
            let linked = Scope {
                project,
                ..s.clone()
            };
            if let Some(e) = self.entries.get_mut(&Self::address(&linked, k)) {
                e.synced.clear();
                e.binding = None;
            }
            self.event(&linked, k, "source-changed");
        }
    }
    pub fn save_app_password(
        &mut self,
        s: &Scope,
        password: &[u8],
        label: &str,
        expires: &str,
        key: &Key,
    ) -> R<()> {
        const FIELD: &str = "APPLE_PASSWORD";
        if self.def(s, FIELD)?.guide != "apple-account"
            || label.chars().count() > 200
            || label.chars().any(char::is_control)
        {
            return Err("invalid".into());
        }
        self.require_owner(s, FIELD)?;
        if password.is_empty() {
            if !expires.is_empty() {
                chrono::NaiveDate::parse_from_str(expires, "%Y-%m-%d").map_err(|_| "date")?;
            }
            let entry = self
                .entries
                .get_mut(&Self::address(s, FIELD))
                .ok_or("missing")?;
            entry.expires = expires.into();
            self.event(s, FIELD, "save");
        } else {
            self.save(s, FIELD, password, expires, key)?;
        }
        self.entries
            .get_mut(&Self::address(s, FIELD))
            .ok_or("missing")?
            .app_password_label = label.trim().into();
        Ok(())
    }
    fn validate_project(name: &str, targets: &[String]) -> R<()> {
        if name.trim().is_empty()
            || name.len() > 120
            || targets.is_empty()
            || targets.len() > 20
            || targets
                .iter()
                .any(|t| !catalog().iter().any(|d| d.targets.contains(t)))
        {
            return Err("invalid".into());
        }
        Ok(())
    }
    pub fn edit_project(&mut self, id: u64, name: &str, targets: Vec<String>) -> R<()> {
        Self::validate_project(name, &targets)?;
        let p = self
            .projects
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or("scope")?;
        p.name = name.trim().into();
        p.targets = targets;
        Ok(())
    }
    pub fn add_project(&mut self, name: &str, targets: Vec<String>) -> R<u64> {
        Self::validate_project(name, &targets)?;
        let id = self.projects.iter().map(|p| p.id).max().unwrap_or(0) + 1;
        self.projects.push(Project {
            id,
            name: name.trim().into(),
            targets,
            custom: vec![],
        });
        Ok(id)
    }
    pub fn custom(&mut self, s: &Scope, key: &str, name: &str, kind: &str) -> R<()> {
        if !key_ok(key)
            || name.trim().is_empty()
            || name.len() > 160
            || !["secret", "config", "id"].contains(&kind)
        {
            return Err("invalid".into());
        }
        let p = self
            .projects
            .iter_mut()
            .find(|p| p.id == s.project && p.targets.contains(&s.target))
            .ok_or("scope")?;
        if catalog().iter().chain(&p.custom).any(|d| d.key == key) {
            return Err("duplicate".into());
        }
        p.custom.push(Def {
            key: key.into(),
            name: name.trim().into(),
            kind: kind.into(),
            group: "Custom".into(),
            guide: "custom".into(),
            targets: vec![s.target.clone()],
        });
        self.event(s, key, "add");
        Ok(())
    }
}
pub fn key_ok(k: &str) -> bool {
    !k.is_empty()
        && k.len() <= 100
        && !k.starts_with("GITHUB_")
        && !k.as_bytes()[0].is_ascii_digit()
        && k.bytes()
            .all(|c| c == b'_' || c.is_ascii_uppercase() || c.is_ascii_digit())
}
