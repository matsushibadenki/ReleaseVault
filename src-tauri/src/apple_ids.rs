// Reconcile after a complete vault transaction so CSV row order and linked inputs are respected.
use crate::model::{Document, Scope, ENVS};
use crate::vault::{self, Key, R};
use serde::{Deserialize, Serialize};

pub const APP_ID: &str = "APPLE_APP_ID";
const TEAM: &str = "APPLE_TEAM_ID";
const BUNDLE: &str = "APPLE_BUNDLE_ID";

#[derive(Clone, Serialize, Deserialize)]
pub enum AppIdPrefix {
    Team,
    Custom(String),
}

impl Document {
    fn identifier(&self, scope: &Scope, field: &str, key: &Key) -> R<Option<String>> {
        let Ok(entry) = self.entry(scope, field) else {
            return Ok(None);
        };
        let bytes = vault::unseal(key, &entry.cipher)?;
        Ok(std::str::from_utf8(&bytes)
            .ok()
            .map(str::trim)
            .filter(|s| !s.is_empty() && s.len() <= 512 && !s.chars().any(char::is_whitespace))
            .map(str::to_owned))
    }

    // Explicitly entered full IDs remain valid even without source fields.
    pub fn entered_app_prefix(
        &self,
        scope: &Scope,
        bytes: &[u8],
        key: &Key,
    ) -> R<Option<AppIdPrefix>> {
        let Some((prefix, suffix)) = std::str::from_utf8(bytes)
            .ok()
            .and_then(|id| id.split_once('.'))
        else {
            return Ok(None);
        };
        if prefix.is_empty() || self.identifier(scope, BUNDLE, key)?.as_deref() != Some(suffix) {
            return Ok(None);
        }
        Ok(Some(
            if Some(prefix) == self.identifier(scope, TEAM, key)?.as_deref() {
                AppIdPrefix::Team
            } else {
                AppIdPrefix::Custom(prefix.to_owned())
            },
        ))
    }

    pub fn refresh_app_ids(&mut self, key: &Key) -> R<()> {
        let targets = crate::model::catalog()
            .iter()
            .find(|def| def.key == APP_ID)
            .expect("bundled App ID");
        let projects: Vec<_> = self
            .projects
            .iter()
            .filter_map(|project| {
                let target = project
                    .targets
                    .iter()
                    .find(|target| targets.targets.contains(target))?;
                Some((project.id, target.clone()))
            })
            .collect();
        for (project, target) in projects {
            for env in ENVS {
                self.refresh_app_id(
                    &Scope {
                        project,
                        target: target.clone(),
                        env: env.into(),
                    },
                    key,
                )?;
            }
        }
        Ok(())
    }

    fn refresh_app_id(&mut self, scope: &Scope, key: &Key) -> R<()> {
        let address = Self::address(scope, APP_ID);
        let existing = self.entries.get(&address);
        // Older manually saved values and IDs linked from other projects own their value.
        let source = match existing {
            Some(entry) => match &entry.app_id_prefix {
                Some(source) if entry.owner.is_none() => source.clone(),
                _ => return Ok(()),
            },
            None => AppIdPrefix::Team,
        };
        let bundle = self.identifier(scope, BUNDLE, key)?;
        let prefix = match &source {
            AppIdPrefix::Team => self.identifier(scope, TEAM, key)?,
            AppIdPrefix::Custom(prefix) => Some(prefix.clone()),
        };
        let (Some(prefix), Some(bundle)) = (prefix, bundle) else {
            // Keep an explicitly chosen prefix across temporary removal of a source field.
            if existing.is_some() && matches!(source, AppIdPrefix::Team) {
                self.entries.remove(&address);
                self.invalidate_linked_metadata(scope, APP_ID);
            }
            return Ok(());
        };
        let id = format!("{prefix}.{bundle}");
        if let Some(entry) = existing {
            if vault::unseal(key, &entry.cipher)?.as_slice() == id.as_bytes() {
                return Ok(());
            }
        }
        let entry = self.entries.entry(address).or_default();
        entry.cipher = vault::seal(key, id.as_bytes())?;
        entry.app_id_prefix = Some(source);
        entry.synced.clear();
        entry.binding = None;
        self.invalidate_linked_metadata(scope, APP_ID);
        Ok(())
    }
}
