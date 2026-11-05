use crate::{
    model::{Document, Entry, Scope},
    vault::R,
};

impl Document {
    /// A reference contains only a project ID. Environment, key name and secrecy type
    /// are fixed when linking. GitHub metadata belongs to the selected project.
    pub fn local_entry(&self, s: &Scope, k: &str) -> R<&Entry> {
        self.def(s, k)?;
        self.entries
            .get(&Self::address(s, k))
            .ok_or_else(|| "missing".into())
    }
    pub fn entry(&self, s: &Scope, k: &str) -> R<&Entry> {
        let local = self.local_entry(s, k)?;
        match local.owner {
            None => Ok(local),
            Some(id) => {
                let source = Scope {
                    project: id,
                    ..s.clone()
                };
                self.entries
                    .get(&Self::address(&source, k))
                    .filter(|e| e.owner.is_none())
                    .ok_or_else(|| "missing".into())
            }
        }
    }
    pub fn dependents(&self, s: &Scope, k: &str) -> Vec<u64> {
        self.projects
            .iter()
            .filter_map(|p| {
                let here = Scope {
                    project: p.id,
                    ..s.clone()
                };
                let e = self.entries.get(&Self::address(&here, k))?;
                (e.owner == Some(s.project)).then_some(p.id)
            })
            .collect()
    }
    pub fn sources(&self, s: &Scope, k: &str) -> Vec<u64> {
        let Ok(def) = self.def(s, k) else {
            return vec![];
        };
        if self.local_entry(s, k).is_ok() || def.group == "Tauri" {
            return vec![];
        }
        self.projects
            .iter()
            .filter(|p| p.id != s.project)
            .filter_map(|p| {
                let source = Scope {
                    project: p.id,
                    ..s.clone()
                };
                let entry = self.entries.get(&Self::address(&source, k))?;
                let compatible = crate::model::catalog().iter().chain(&p.custom).any(|src| {
                    src.key == k
                        && src.kind == def.kind
                        && src.targets.iter().any(|t| p.targets.contains(t))
                });
                (entry.owner.is_none() && compatible).then_some(p.id)
            })
            .collect()
    }
    pub fn link(&mut self, s: &Scope, k: &str, owner: u64) -> R<()> {
        if !self.sources(s, k).contains(&owner) {
            return Err("share".into());
        }
        self.entries.insert(
            Self::address(s, k),
            Entry {
                owner: Some(owner),
                ..Default::default()
            },
        );
        self.event(s, k, "link");
        self.event(
            &Scope {
                project: owner,
                ..s.clone()
            },
            k,
            "shared",
        );
        Ok(())
    }
    pub fn unlink(&mut self, s: &Scope, k: &str) -> R<()> {
        let owner = self.local_entry(s, k)?.owner.ok_or("share")?;
        self.entries.remove(&Self::address(s, k));
        self.event(s, k, "unlink");
        self.event(
            &Scope {
                project: owner,
                ..s.clone()
            },
            k,
            "unshared",
        );
        Ok(())
    }
    pub fn require_owner(&self, s: &Scope, k: &str) -> R<()> {
        if self.local_entry(s, k).ok().and_then(|e| e.owner).is_some() {
            return Err("shared-readonly".into());
        }
        Ok(())
    }
    pub fn clear_value(&mut self, s: &Scope, k: &str) -> R<()> {
        self.require_owner(s, k)?;
        self.local_entry(s, k)?;
        if !self.dependents(s, k).is_empty() {
            return Err("shared-in-use".into());
        }
        self.entries.remove(&Self::address(s, k));
        self.event(s, k, "clear");
        Ok(())
    }
}
