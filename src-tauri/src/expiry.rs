use crate::model::{Def, Document, Entry, Scope, ENVS};
use chrono::NaiveDate;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Invalid,
    Expired,
    Soon,
    Unchecked,
    Unknown,
    Valid,
}
#[derive(Serialize)]
pub struct ProjectUse {
    pub id: u64,
    pub name: String,
}
#[derive(Serialize)]
pub struct Item {
    pub scope: Scope,
    pub def: Def,
    pub source: String,
    pub source_id: u64,
    pub projects: Vec<ProjectUse>,
    pub expires: String,
    pub days: Option<i64>,
    pub checked: Option<String>,
    pub flags: Vec<State>,
    pub state: State,
}
#[derive(Serialize)]
pub struct Overview {
    pub as_of: String,
    pub items: Vec<Item>,
}
fn apple_item(def: &Def) -> bool {
    def.guide != "custom"
        && matches!(
            def.key.as_str(),
            "APPLE_CERTIFICATE"
                | "APPLE_STORE_CERTIFICATE"
                | "APPLE_INSTALLER_CERTIFICATE"
                | "APPLE_PROVISIONING_PROFILE"
        )
}
fn tracked(def: &Def, entry: &Entry) -> bool {
    !entry.expires.is_empty()
        || entry.apple_check.is_some()
        || apple_item(def)
        || (def.guide != "custom"
            && matches!(
                def.key.as_str(),
                "WINDOWS_PFX" | "ANDROID_KEYSTORE" | "PLAY_UPLOAD_KEYSTORE"
            ))
}
fn assess(def: &Def, entry: &Entry, today: NaiveDate) -> (Vec<State>, Option<i64>) {
    let mut flags = Vec::new();
    let days = NaiveDate::parse_from_str(&entry.expires, "%Y-%m-%d")
        .ok()
        .map(|date| (date - today).num_days());
    if entry
        .apple_check
        .as_ref()
        .is_some_and(|check| check.kind == "profiles" && check.state != "ACTIVE")
    {
        flags.push(State::Invalid);
    }
    if let Some(days) = days {
        if days < 0 {
            flags.push(State::Expired);
        } else if days <= 30 {
            flags.push(State::Soon);
        }
    }
    if apple_item(def) && entry.apple_check.is_none() {
        flags.push(State::Unchecked);
    }
    if days.is_none() {
        flags.push(State::Unknown);
    }
    if flags.is_empty() {
        flags.push(State::Valid);
    }
    (flags, days)
}
pub fn overview(d: &Document, current: &Scope, today: NaiveDate) -> Overview {
    // Entries are shared across distribution targets. Group references by the real owning entry.
    let mut items: BTreeMap<(u64, String, String), Item> = BTreeMap::new();
    for project in &d.projects {
        for env in ENVS {
            let mut seen = BTreeSet::new();
            let mut targets = project.targets.clone();
            if project.id == current.project && targets.contains(&current.target) {
                targets.retain(|target| target != &current.target);
                targets.insert(0, current.target.clone());
            }
            for target in targets {
                let scope = Scope {
                    project: project.id,
                    target,
                    env: env.into(),
                };
                let Ok(defs) = d.defs(&scope) else { continue };
                for def in defs {
                    if !seen.insert(def.key.clone()) {
                        continue;
                    }
                    let Ok(entry) = d.entry(&scope, &def.key) else {
                        continue;
                    };
                    if !tracked(def, entry) {
                        continue;
                    }
                    let owner = d
                        .local_entry(&scope, &def.key)
                        .ok()
                        .and_then(|local| local.owner)
                        .unwrap_or(project.id);
                    let identity = (owner, env.to_owned(), def.key.clone());
                    let item = items.entry(identity).or_insert_with(|| {
                        let (flags, days) = assess(def, entry, today);
                        Item {
                            scope: scope.clone(),
                            def: def.clone(),
                            source_id: owner,
                            source: d
                                .projects
                                .iter()
                                .find(|project| project.id == owner)
                                .map(|project| project.name.clone())
                                .unwrap_or_else(|| owner.to_string()),
                            projects: vec![],
                            expires: entry.expires.clone(),
                            days,
                            checked: entry
                                .apple_check
                                .as_ref()
                                .map(|check| check.checked.clone()),
                            state: flags[0],
                            flags,
                        }
                    });
                    if owner == project.id {
                        item.scope = scope.clone();
                        item.def = def.clone();
                        let (flags, days) = assess(def, entry, today);
                        item.state = flags[0];
                        item.flags = flags;
                        item.days = days;
                    }
                    item.projects.push(ProjectUse {
                        id: project.id,
                        name: project.name.clone(),
                    });
                }
            }
        }
    }
    let mut items: Vec<_> = items.into_values().collect();
    items.sort_by(|a, b| {
        a.state
            .cmp(&b.state)
            .then(a.days.unwrap_or(i64::MAX).cmp(&b.days.unwrap_or(i64::MAX)))
            .then(a.source.cmp(&b.source))
            .then(a.scope.env.cmp(&b.scope.env))
            .then(a.def.key.cmp(&b.def.key))
    });
    Overview {
        as_of: today.to_string(),
        items,
    }
}
