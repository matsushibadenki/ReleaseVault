//! One authenticated document per SQLite database. Only encrypted envelopes reach disk.
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
        serde_json::from_slice(&unseal(k, &self.body)?).map_err(|_| "damaged".into())
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
        db.execute_batch("PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS vault(id INTEGER PRIMARY KEY CHECK(id=1), revision INTEGER NOT NULL, envelope BLOB NOT NULL);").map_err(|_|"storage")?;
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
    pub fn reauth(&mut self, p: &str) -> R<()> {
        self.unlocked()?;
        self.envelope()?.ok_or("missing")?.1.key(p)?;
        Ok(())
    }
    pub fn change<T>(&mut self, f: impl FnOnce(&mut Document, &Key) -> R<T>) -> R<T> {
        let s = self.unlocked()?;
        let mut d = s.doc.clone();
        let result = f(&mut d, &s.key)?;
        let rev = s.revision;
        let body = seal(
            &s.key,
            &Zeroizing::new(serde_json::to_vec(&d).map_err(|_| "storage")?),
        )?;
        let (_, mut e) = self.envelope()?.ok_or("missing")?;
        e.body = body;
        let n = self
            .db
            .execute(
                "UPDATE vault SET revision=revision+1,envelope=? WHERE id=1 AND revision=?",
                params![serde_json::to_vec(&e).map_err(|_| "storage")?, rev],
            )
            .map_err(|_| "storage")?;
        if n != 1 {
            self.session = None;
            return Err("conflict".into());
        }
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
        let r = self.change(|d, _| {
            if crate::integrations::fetch_remote(d, &scope, &k).is_err() {
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
