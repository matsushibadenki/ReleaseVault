//! Fixed-origin REST transport. Credentials, response bodies and headers never become UI errors.
use crate::vault::R;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use openssl::{hash::MessageDigest, pkey::PKey, rsa::Padding, sign::Signer};
use serde_json::{json, Value};
use std::{io::Read, time::Duration};
use zeroize::Zeroizing;

pub(super) struct Api {
    agent: ureq::Agent,
    origin: String,
}
pub(super) struct Reply {
    pub status: u16,
    pub data: Value,
}
impl Reply {
    pub fn success(self) -> R<Value> {
        match self.status {
            200..=299 => Ok(self.data),
            401 => Err("app-auth".into()),
            403 | 429 => Err("app-denied".into()),
            404 => Err("app-missing".into()),
            422 => Err("app-grant".into()),
            _ => Err("app-api".into()),
        }
    }
}
impl Api {
    pub fn github() -> Self {
        Self::at("https://api.github.com")
    }
    fn at(origin: &str) -> Self {
        Self {
            agent: ureq::AgentBuilder::new()
                .redirects(0)
                .timeout(Duration::from_secs(15))
                .build(),
            origin: origin.into(),
        }
    }
    #[cfg(test)]
    pub fn fixture(origin: &str) -> Self {
        assert!(origin.starts_with("http://127.0.0.1:"));
        Self::at(origin)
    }
    pub fn request(&self, method: &str, path: &str, token: &str, body: Option<Value>) -> R<Reply> {
        let req = self
            .agent
            .request(method, &format!("{}{path}", self.origin))
            .set("User-Agent", "ReleaseVault")
            .set("Authorization", &format!("Bearer {token}"))
            .set("Accept", "application/vnd.github+json")
            .set("X-GitHub-Api-Version", "2026-03-10");
        let response = match body {
            Some(b) => req.send_json(b),
            None => req.call(),
        };
        let response = match response {
            Ok(r) => r,
            Err(ureq::Error::Status(s, _)) => {
                return Ok(Reply {
                    status: s,
                    data: Value::Null,
                })
            }
            Err(_) => return Err("app-network".into()),
        };
        let status = response.status();
        let mut bytes = Zeroizing::new(Vec::new());
        response
            .into_reader()
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "app-network")?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err("app-api".into());
        }
        let data = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).map_err(|_| "app-api")?
        };
        Ok(Reply { status, data })
    }
    pub fn token(
        &self,
        jwt: &str,
        install: u64,
        repo: Option<u64>,
        permission: &str,
        level: &str,
    ) -> R<Zeroizing<String>> {
        let mut body = json!({"permissions":{(permission):level}});
        if let Some(id) = repo {
            body["repository_ids"] = json!([id]);
        }
        let mut response = self
            .request(
                "POST",
                &format!("/app/installations/{install}/access_tokens"),
                jwt,
                Some(body),
            )?
            .success()?;
        match response["token"].take() {
            Value::String(s) if !s.is_empty() => Ok(Zeroizing::new(s)),
            _ => Err("app-api".into()),
        }
    }
}
pub(super) fn jwt(app: u64, pem: &[u8]) -> R<Zeroizing<String>> {
    // An explicit empty passphrase prevents OpenSSL from opening a terminal prompt.
    let key = PKey::private_key_from_pem_passphrase(pem, b"").map_err(|_| "app-key")?;
    if key.id() != openssl::pkey::Id::RSA || key.bits() < 2048 {
        return Err("app-key".into());
    }
    let now = chrono::Utc::now().timestamp();
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256","typ":"JWT"}"#);
    let claims = URL_SAFE_NO_PAD
        .encode(json!({"iss":app.to_string(),"iat":now-60,"exp":now+480}).to_string());
    let payload = format!("{header}.{claims}");
    let mut signer = Signer::new(MessageDigest::sha256(), &key).map_err(|_| "app-key")?;
    signer
        .set_rsa_padding(Padding::PKCS1)
        .map_err(|_| "app-key")?;
    signer.update(payload.as_bytes()).map_err(|_| "app-key")?;
    let signature = signer.sign_to_vec().map_err(|_| "app-key")?;
    Ok(Zeroizing::new(format!(
        "{payload}.{}",
        URL_SAFE_NO_PAD.encode(signature)
    )))
}
