//! One authenticated document per SQLite database. Only encrypted envelopes reach disk.
use crate::device::Method;
use crate::model::Document;
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    XChaCha20Poly1305, XNonce,
};
use rand::{rngs::OsRng, RngCore};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

pub type R<T> = Result<T, String>;
pub type Key = Zeroizing<[u8; 32]>;
pub fn random<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    OsRng.fill_bytes(&mut b);
    b
}
fn derive(p: &str, salt: &[u8]) -> R<Key> {
    if salt.len() != 16 || p.len() > 4096 {
        return Err("invalid".into());
    }
    let mut k = Zeroizing::new([0; 32]);
    let params = Params::new(64 * 1024, 3, 1, Some(32)).map_err(|_| "crypto")?;
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(p.as_bytes(), salt, k.as_mut())
        .map_err(|_| "crypto")?;
    Ok(k)
}
pub fn seal(k: &Key, b: &[u8]) -> R<Vec<u8>> {
    let n = random::<24>();
    let c = XChaCha20Poly1305::new((&**k).into())
        .encrypt(XNonce::from_slice(&n), b)
        .map_err(|_| "crypto")?;
    Ok([n.as_slice(), &c].concat())
}
pub fn unseal(k: &Key, b: &[u8]) -> R<Zeroizing<Vec<u8>>> {
    if b.len() < 40 {
        return Err("damaged".into());
    }
    XChaCha20Poly1305::new((&**k).into())
        .decrypt(XNonce::from_slice(&b[..24]), &b[24..])
        .map(Zeroizing::new)
        .map_err(|_| "auth".into())
}
#[derive(Serialize, Deserialize)]
pub struct Envelope {
    format: String,
    salt: Vec<u8>,
    wrapped: Vec<u8>,
    #[serde(with = "blob")]
    body: Vec<u8>,
}
impl Envelope {
    fn key(&self, p: &str) -> R<Key> {
        if self.format != "ReleaseVault/1" {
            return Err("format".into());
        }
        let raw = unseal(&derive(p, &self.salt)?, &self.wrapped)?;
        Ok(Zeroizing::new(
            raw.as_slice().try_into().map_err(|_| "damaged")?,
        ))
    }
    fn document(&self, k: &Key) -> R<Document> {
        let mut doc: Document =
            serde_json::from_slice(&unseal(k, &self.body)?).map_err(|_| "damaged")?;
        // Older vaults could define these keys themselves. Entries are addressed by key.
        for project in &mut doc.projects {
            project.custom.retain_mut(|def| {
                if !matches!(
                    def.key.as_str(),
                    "APPLE_ID" | "APPLE_PASSWORD" | "APPLE_ACCOUNT_PASSWORD"
                ) {
                    return true;
                }
                if def.kind != "secret" || def.key == "APPLE_ACCOUNT_PASSWORD" {
                    for env in crate::model::ENVS {
                        let address = format!("{}/{env}/{}", project.id, def.key);
                        if let Some(entry) = doc.entries.get_mut(&address) {
                            entry.binding = None;
                            entry.synced.clear();
                        }
                    }
                    def.kind = "secret".into();
                }
                if let Some(builtin) = crate::model::catalog()
                    .iter()
                    .find(|item| item.key == def.key)
                {
                    def.targets
                        .retain(|target| !builtin.targets.contains(target));
                }
                !def.targets.is_empty()
            });
        }
        doc.refresh_app_ids(k)?;
        Ok(doc)
    }
}
pub struct Session {
    pub key: Key,
    pub doc: Document,
    revision: i64,
    last: Instant,
}
pub struct Vault {
    db: Connection,
    pub session: Option<Session>,
}
impl Vault {
    pub fn open(path: &Path) -> R<Self> {
        let db = Connection::open(path).map_err(|_| "storage")?;
        db.execute_batch("PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS vault(id INTEGER PRIMARY KEY CHECK(id=1), revision INTEGER NOT NULL, envelope BLOB NOT NULL); CREATE TABLE IF NOT EXISTS device_unlock(id INTEGER PRIMARY KEY CHECK(id=1), record BLOB NOT NULL); CREATE TABLE IF NOT EXISTS passkey_unlock(id INTEGER PRIMARY KEY CHECK(id=1), record BLOB NOT NULL);").map_err(|_|"storage")?;
        Ok(Self { db, session: None })
    }
    fn envelope(&self) -> R<Option<(i64, Envelope)>> {
        let r: Option<(i64, Vec<u8>)> = self
            .db
            .query_row("SELECT revision,envelope FROM vault WHERE id=1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()
            .map_err(|_| "storage")?;
        r.map(|(n, b)| Ok((n, serde_json::from_slice(&b).map_err(|_| "damaged")?)))
            .transpose()
    }
    pub fn exists(&self) -> R<bool> {
        self.db
            .query_row("SELECT EXISTS(SELECT 1 FROM vault)", [], |r| r.get(0))
            .map_err(|_| "storage".into())
    }
    pub fn expire(&mut self) {
        if self
            .session
            .as_ref()
            .is_some_and(|s| s.last.elapsed() >= Duration::from_secs(300))
        {
            self.session = None
        }
    }
    pub fn unlocked(&mut self) -> R<&mut Session> {
        self.expire();
        let s = self.session.as_mut().ok_or("locked")?;
        s.last = Instant::now();
        Ok(s)
    }
    pub fn create(&mut self, p: &str) -> R<()> {
        if p.chars().count() < 12 {
            return Err("weak".into());
        }
        let salt = random::<16>();
        let key = Zeroizing::new(random());
        let doc = Document::initial();
        let env = Envelope {
            format: "ReleaseVault/1".into(),
            salt: salt.to_vec(),
            wrapped: seal(&derive(p, &salt)?, key.as_slice())?,
            body: seal(
                &key,
                &Zeroizing::new(serde_json::to_vec(&doc).map_err(|_| "storage")?),
            )?,
        };
        self.insert(&env)?;
        self.session = Some(Session {
            key,
            doc,
            revision: 1,
            last: Instant::now(),
        });
        Ok(())
    }
    fn insert(&self, env: &Envelope) -> R<()> {
        self.db
            .execute(
                "INSERT INTO vault VALUES (1,1,?1)",
                [serde_json::to_vec(env).map_err(|_| "storage")?],
            )
            .map_err(|_| "exists")?;
        Ok(())
    }
    pub fn unlock(&mut self, p: &str) -> R<()> {
        let (revision, e) = self.envelope()?.ok_or("missing")?;
        let key = e.key(p)?;
        let doc = e.document(&key)?;
        self.session = Some(Session {
            key,
            doc,
            revision,
            last: Instant::now(),
        });
        Ok(())
    }
    pub fn method_enabled(&self, method: Method) -> R<bool> {
        self.db
            .query_row(
                &format!("SELECT EXISTS(SELECT 1 FROM {})", method.table()),
                [],
                |r| r.get(0),
            )
            .map_err(|_| "storage".into())
    }
    pub fn set_method_record(&mut self, method: Method, record: Option<&[u8]>) -> R<()> {
        self.unlocked()?;
        match record {
            Some(bytes) => self.db.execute(
                &format!("INSERT OR REPLACE INTO {} VALUES (1,?)", method.table()),
                [bytes],
            ),
            None => self
                .db
                .execute(&format!("DELETE FROM {}", method.table()), []),
        }
        .map_err(|_| "storage")?;
        Ok(())
    }
    pub fn method_record(&self, method: Method) -> R<Vec<u8>> {
        self.db
            .query_row(
                &format!("SELECT record FROM {} WHERE id=1", method.table()),
                [],
                |r| r.get(0),
            )
            .map_err(|_| method.error().into())
    }
    pub fn finish_method_unlock(&mut self, method: Method, record: &[u8], key: Key) -> R<()> {
        if self.method_record(method)? != record {
            return Err(method.error().into());
        }
        let (revision, envelope) = self.envelope()?.ok_or("missing")?;
        let doc = envelope.document(&key)?;
        self.session = Some(Session {
            key,
            doc,
            revision,
            last: Instant::now(),
        });
        Ok(())
    }
    pub fn reauth(&mut self, p: &str) -> R<()> {
        self.unlocked()?;
        self.envelope()?.ok_or("missing")?.1.key(p)?;
        Ok(())
    }
    pub fn change<T>(&mut self, f: impl FnOnce(&mut Document, &Key) -> R<T>) -> R<T> {
        self.unlocked()?;
        let tx = self
            .db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| "busy")?;
        let (stored, envelope): (i64, Vec<u8>) = tx
            .query_row(
                "SELECT revision,envelope FROM vault WHERE id=1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|_| "storage")?;
        let s = self.session.as_ref().ok_or("locked")?;
        if s.revision != stored {
            self.session = None;
            return Err("conflict".into());
        }
        let mut e: Envelope = serde_json::from_slice(&envelope).map_err(|_| "damaged")?;
        let mut d = s.doc.clone();
        let result = f(&mut d, &s.key)?;
        d.refresh_app_ids(&s.key)?;
        let rev = s.revision;
        let body = seal(
            &s.key,
            &Zeroizing::new(serde_json::to_vec(&d).map_err(|_| "storage")?),
        )?;
        e.body = body;
        let bytes = serde_json::to_vec(&e).map_err(|_| "storage")?;
        if bytes.len() > 128 * 1024 * 1024 {
            return Err("size".into());
        }
        let n = tx
            .execute(
                "UPDATE vault SET revision=revision+1,envelope=? WHERE id=1 AND revision=?",
                params![bytes, rev],
            )
            .map_err(|_| "storage")?;
        if n != 1 {
            self.session = None;
            return Err("conflict".into());
        }
        tx.commit().map_err(|_| "storage")?;
        let s = self.session.as_mut().unwrap();
        s.doc = d;
        s.revision += 1;
        Ok(result)
    }
    pub fn backup(&mut self, p: &str) -> R<Vec<u8>> {
        self.reauth(p)?;
        serde_json::to_vec(&self.envelope()?.ok_or("missing")?.1).map_err(|_| "storage".into())
    }
    pub fn restore(&mut self, b: &[u8], p: &str) -> R<()> {
        if self.exists()? {
            return Err("exists".into());
        }
        let e: Envelope = serde_json::from_slice(b).map_err(|_| "format")?;
        let key = e.key(p)?;
        let doc = e.document(&key)?;
        if doc.projects.is_empty() {
            return Err("format".into());
        }
        self.insert(&e)?;
        self.session = Some(Session {
            key,
            doc,
            revision: 1,
            last: Instant::now(),
        });
        Ok(())
    }
}
impl Vault {
    // Poll at most one binding per tick. A background read is never user activity.
    pub fn refresh_due(&mut self) -> R<bool> {
        self.expire();
        let Some(s) = self.session.as_ref() else {
            return Ok(false);
        };
        let last = s.last;
        let now = chrono::Utc::now();
        let due = s
            .doc
            .entries
            .iter()
            .filter_map(|(addr, e)| {
                let b = e.binding.as_ref()?;
                if !b.automatic {
                    return None;
                }
                let stamp = chrono::DateTime::parse_from_rfc3339(&b.polled).ok();
                if stamp.is_some_and(|x| now.signed_duration_since(x).num_minutes() < 15) {
                    return None;
                }
                let mut p = addr.split('/');
                let id = p.next()?.parse::<u64>().ok()?;
                let env = p.next()?;
                let k = p.next()?;
                let proj = s.doc.projects.iter().find(|v| v.id == id)?;
                let target = proj.targets.iter().find(|t| {
                    crate::model::catalog()
                        .iter()
                        .chain(&proj.custom)
                        .any(|d| d.key == k && d.targets.contains(t))
                })?;
                Some((
                    b.polled.clone(),
                    crate::model::Scope {
                        project: id,
                        target: target.clone(),
                        env: env.into(),
                    },
                    k.to_string(),
                ))
            })
            .min_by(|a, b| a.0.cmp(&b.0));
        let Some((_, scope, k)) = due else {
            return Ok(false);
        };
        let r = self.change(|d, key| {
            if crate::integrations::fetch_remote(d, &scope, &k, key).is_err() {
                let b = d
                    .entries
                    .get_mut(&crate::model::Document::address(&scope, &k))
                    .and_then(|e| e.binding.as_mut())
                    .ok_or("missing")?;
                b.polled = now.to_rfc3339();
                b.remote = "unavailable".into()
            }
            Ok(())
        });
        if let Some(s) = &mut self.session {
            s.last = last
        }
        self.expire();
        r.map(|_| true)
    }
}

/// Compact ciphertext encoding; a single maximum-size import must remain portable.
pub mod blob {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&STANDARD.encode(bytes))
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        STANDARD
            .decode(String::deserialize(d)?)
            .map_err(serde::de::Error::custom)
    }
}
