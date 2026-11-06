use crate::{
    integrations,
    model::{Document, Scope},
    vault::{self, Key, R},
};
use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine,
};
use openssl::{
    ecdsa::EcdsaSig, hash::MessageDigest, nid::Nid, pkcs12::Pkcs12, pkey::PKey, sign::Signer,
    x509::X509,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    io::Read,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

const ORIGIN: &str = "https://api.appstoreconnect.apple.com";
const CERTS:&str="/v1/certificates?limit=50&fields%5Bcertificates%5D=name,certificateType,displayName,expirationDate,certificateContent";
const PROFILES:&str="/v1/profiles?limit=50&fields%5Bprofiles%5D=name,profileType,profileState,uuid,expirationDate,profileContent";
const AUTH_FIELDS: [&str; 3] = ["APPLE_API_ISSUER", "APPLE_API_KEY", "APPLE_API_KEY_P8"];

pub struct Credentials {
    issuer: Zeroizing<String>,
    id: Zeroizing<String>,
    pem: Zeroizing<Vec<u8>>,
}
impl Credentials {
    pub fn load(d: &Document, s: &Scope, key: &Key) -> R<Self> {
        let text = |field: &str| -> R<Zeroizing<String>> {
            let bytes = integrations::value(d, s, field, key).map_err(|_| "apple-config")?;
            Ok(Zeroizing::new(
                std::str::from_utf8(&bytes)
                    .map_err(|_| "apple-config")?
                    .trim()
                    .into(),
            ))
        };
        let issuer = text(AUTH_FIELDS[0])?;
        let groups: Vec<_> = issuer.split('-').collect();
        if groups.iter().map(|g| g.len()).collect::<Vec<_>>() != [8, 4, 4, 4, 12]
            || groups
                .iter()
                .any(|g| !g.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err("apple-config".into());
        }
        let id = text(AUTH_FIELDS[1])?;
        if id.is_empty() || id.len() > 128 || !id.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err("apple-config".into());
        }
        let stored = integrations::value(d, s, AUTH_FIELDS[2], key).map_err(|_| "apple-config")?;
        let pem = if stored.trim_ascii().starts_with(b"-----BEGIN") {
            Zeroizing::new(stored.trim_ascii().to_vec())
        } else {
            Zeroizing::new(
                STANDARD
                    .decode(stored.trim_ascii())
                    .map_err(|_| "apple-key")?,
            )
        };
        Ok(Self { issuer, id, pem })
    }
    pub fn fingerprint(&self) -> String {
        let mut hash = openssl::sha::Sha256::new();
        for bytes in [
            self.issuer.as_bytes(),
            self.id.as_bytes(),
            self.pem.as_slice(),
        ] {
            hash.update(&(bytes.len() as u64).to_be_bytes());
            hash.update(bytes);
        }
        hex(&hash.finish())
    }
    fn token(&self) -> R<Zeroizing<String>> {
        let key = PKey::private_key_from_pem_passphrase(&self.pem, b"").map_err(|_| "apple-key")?;
        let ec = key.ec_key().map_err(|_| "apple-key")?;
        if ec.group().curve_name() != Some(Nid::X9_62_PRIME256V1) {
            return Err("apple-key".into());
        }
        let now = chrono::Utc::now().timestamp();
        let header =
            URL_SAFE_NO_PAD.encode(json!({"alg":"ES256","kid":&*self.id,"typ":"JWT"}).to_string());
        let claims = URL_SAFE_NO_PAD.encode(
            json!({"iss":&*self.issuer,"iat":now-30,"exp":now+300,
            "aud":"appstoreconnect-v1","scope":[format!("GET {CERTS}"),format!("GET {PROFILES}")]})
            .to_string(),
        );
        let payload = format!("{header}.{claims}");
        let mut signer = Signer::new(MessageDigest::sha256(), &key).map_err(|_| "apple-key")?;
        signer.update(payload.as_bytes()).map_err(|_| "apple-key")?;
        let signature = EcdsaSig::from_der(&signer.sign_to_vec().map_err(|_| "apple-key")?)
            .map_err(|_| "apple-key")?;
        let mut raw = signature.r().to_vec_padded(32).map_err(|_| "apple-key")?;
        raw.extend(signature.s().to_vec_padded(32).map_err(|_| "apple-key")?);
        Ok(Zeroizing::new(format!(
            "{payload}.{}",
            URL_SAFE_NO_PAD.encode(raw)
        )))
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}
fn fingerprint(bytes: &[u8]) -> String {
    hex(&openssl::sha::sha256(bytes))
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Resource {
    pub kind: String,
    pub id: String,
    pub name: String,
    pub category: String,
    pub expires: String,
    pub state: String,
    pub fingerprint: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Report {
    pub id: String,
    pub checked: String,
    pub credentials: String,
    pub resources: Vec<Resource>,
}
impl Report {
    pub fn address(s: &Scope) -> String {
        format!("{}/{}", s.project, s.env)
    }
    pub fn view(&self, d: &Document, s: &Scope, key: &Key) -> Value {
        let current =
            Credentials::load(d, s, key).is_ok_and(|auth| auth.fingerprint() == self.credentials);
        json!({"id":self.id,"checked":self.checked,"credentialsCurrent":current,"resources":self.resources.iter().map(|resource|
            json!({"kind":resource.kind,"id":resource.id,"name":resource.name,"category":resource.category,"expires":resource.expires,"state":resource.state})).collect::<Vec<_>>()})
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub kind: String,
    pub id: String,
    pub name: String,
    pub checked: String,
    pub state: String,
}
pub struct Api {
    origin: String,
    agent: ureq::Agent,
}
impl Api {
    pub fn apple() -> Self {
        Self::at(ORIGIN)
    }
    fn at(origin: &str) -> Self {
        Self {
            origin: origin.into(),
            agent: ureq::AgentBuilder::new()
                .redirects(0)
                .timeout(Duration::from_secs(15))
                .build(),
        }
    }
    fn get(&self, path: &str, token: &str) -> R<Value> {
        let response = match self
            .agent
            .get(&format!("{}{path}", self.origin))
            .set("Authorization", &format!("Bearer {token}"))
            .set("User-Agent", "ReleaseVault")
            .call()
        {
            Ok(response) => response,
            Err(ureq::Error::Status(401, _)) => return Err("apple-auth".into()),
            Err(ureq::Error::Status(403, _)) => return Err("apple-denied".into()),
            Err(ureq::Error::Status(429, _)) => return Err("apple-rate".into()),
            Err(_) => return Err("apple-network".into()),
        };
        if response.status() != 200 {
            return Err("apple-network".into());
        }
        let mut bytes = Zeroizing::new(Vec::new());
        response
            .into_reader()
            .take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "apple-network")?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err("apple-limit".into());
        }
        serde_json::from_slice(&bytes).map_err(|_| "apple-response".into())
    }
}
fn next_page(raw: &str, kind: &str) -> R<String> {
    let url = tauri::Url::parse(raw).map_err(|_| "apple-response")?;
    if url.origin().ascii_serialization() != ORIGIN
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.path() != format!("/v1/{kind}")
    {
        return Err("apple-response".into());
    }
    Ok(format!("{}?{}", url.path(), url.query().unwrap_or("")))
}
pub fn fetch(api: &Api, auth: &Credentials, validate: impl Fn() -> R<()>) -> R<Report> {
    let token = auth.token()?;
    let start = Instant::now();
    let mut resources = Vec::new();
    for (kind, initial) in [("certificates", CERTS), ("profiles", PROFILES)] {
        let mut path = initial.to_owned();
        let mut seen = BTreeSet::new();
        let mut ids = BTreeSet::new();
        for page in 0..20 {
            validate()?;
            if start.elapsed() > Duration::from_secs(90) || !seen.insert(path.clone()) {
                return Err("apple-limit".into());
            }
            let data = api.get(&path, &token)?;
            for row in data["data"].as_array().ok_or("apple-response")? {
                let resource = resource(row, kind)?;
                if !ids.insert(resource.id.clone()) {
                    return Err("apple-response".into());
                }
                resources.push(resource);
                if resources.len() > 2000 {
                    return Err("apple-limit".into());
                }
            }
            let next = data["links"]["next"].as_str().filter(|s| !s.is_empty());
            match next {
                None => break,
                Some(_) if page == 19 => return Err("apple-limit".into()),
                Some(next) => path = next_page(next, kind)?,
            }
        }
    }
    validate()?;
    resources.sort_by(|a, b| a.expires.cmp(&b.expires).then(a.name.cmp(&b.name)));
    Ok(Report {
        id: STANDARD.encode(vault::random::<24>()),
        checked: chrono::Utc::now().to_rfc3339(),
        credentials: auth.fingerprint(),
        resources,
    })
}
fn resource(row: &Value, kind: &str) -> R<Resource> {
    if row["type"] != kind {
        return Err("apple-response".into());
    }
    let attrs = &row["attributes"];
    let text = |value: &Value| -> R<String> {
        let value = value
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 1024)
            .ok_or("apple-response")?;
        Ok(value.into())
    };
    let id = text(&row["id"])?;
    let expires = chrono::DateTime::parse_from_rfc3339(
        attrs["expirationDate"].as_str().ok_or("apple-response")?,
    )
    .map_err(|_| "apple-response")?
    .with_timezone(&chrono::Utc)
    .format("%Y-%m-%d")
    .to_string();
    let field = if kind == "certificates" {
        "certificateContent"
    } else {
        "profileContent"
    };
    let bytes = Zeroizing::new(
        STANDARD
            .decode(attrs[field].as_str().ok_or("apple-response")?)
            .map_err(|_| "apple-response")?,
    );
    if bytes.is_empty() || bytes.len() > 1024 * 1024 {
        return Err("apple-response".into());
    }
    if kind == "certificates" {
        let (certificate_expiry, _) =
            integrations::certificate(&bytes).map_err(|_| "apple-response")?;
        if certificate_expiry != expires {
            return Err("apple-response".into());
        }
    }
    Ok(Resource {
        kind: kind.into(),
        id,
        name: attrs["name"]
            .as_str()
            .or_else(|| attrs["displayName"].as_str())
            .filter(|s| !s.is_empty() && s.len() <= 1024)
            .unwrap_or("—")
            .into(),
        category: text(
            &attrs[if kind == "certificates" {
                "certificateType"
            } else {
                "profileType"
            }],
        )?,
        expires,
        state: if kind == "profiles" {
            text(&attrs["profileState"])?
        } else {
            "PRESENT".into()
        },
        fingerprint: fingerprint(&bytes),
    })
}
fn local_certificate(d: &Document, s: &Scope, field: &str, key: &Key) -> R<String> {
    let entry = d.entry(s, field)?;
    if !entry.matched_fingerprint.is_empty() {
        return Ok(entry.matched_fingerprint.clone());
    }
    let value = integrations::value(d, s, field, key)?;
    let bytes = if value.trim_ascii().starts_with(b"-----BEGIN") {
        Zeroizing::new(value.trim_ascii().to_vec())
    } else {
        Zeroizing::new(
            STANDARD
                .decode(value.trim_ascii())
                .map_err(|_| "apple-file")?,
        )
    };
    let cert = if let Ok(cert) = X509::from_pem(&bytes).or_else(|_| X509::from_der(&bytes)) {
        cert
    } else {
        let password_field = match field {
            "APPLE_CERTIFICATE" => "APPLE_CERTIFICATE_PASSWORD",
            "APPLE_STORE_CERTIFICATE" => "APPLE_STORE_CERTIFICATE_PASSWORD",
            "APPLE_INSTALLER_CERTIFICATE" => "APPLE_INSTALLER_PASSWORD",
            _ => return Err("apple-file".into()),
        };
        let password = match integrations::value(d, s, password_field, key) {
            Ok(value) => value,
            Err(_) => Zeroizing::new(Vec::new()),
        };
        let password = std::str::from_utf8(&password).map_err(|_| "apple-file")?;
        if password.contains('\0') {
            return Err("apple-file".into());
        }
        Pkcs12::from_der(&bytes)
            .map_err(|_| "apple-file")?
            .parse2(password)
            .map_err(|_| "apple-file")?
            .cert
            .ok_or("apple-file")?
    };
    Ok(fingerprint(&cert.to_der().map_err(|_| "apple-file")?))
}
pub fn apply(
    d: &mut Document,
    s: &Scope,
    field: &str,
    report_id: &str,
    resource_id: &str,
    kind: &str,
    key: &Key,
) -> R<()> {
    d.require_owner(s, field)?;
    let report = d
        .apple_reports
        .get(&Report::address(s))
        .filter(|r| r.id == report_id)
        .ok_or("apple-stale")?;
    if Credentials::load(d, s, key)?.fingerprint() != report.credentials {
        return Err("apple-stale".into());
    }
    let resource = report
        .resources
        .iter()
        .find(|r| r.id == resource_id && r.kind == kind)
        .ok_or("apple-stale")?
        .clone();
    let certificate = matches!(
        field,
        "APPLE_CERTIFICATE" | "APPLE_STORE_CERTIFICATE" | "APPLE_INSTALLER_CERTIFICATE"
    );
    if d.def(s, field)?.guide == "custom"
        || (!certificate && field != "APPLE_PROVISIONING_PROFILE")
        || (certificate && kind != "certificates")
        || (!certificate && kind != "profiles")
    {
        return Err("apple-file".into());
    }
    let local = if certificate {
        local_certificate(d, s, field, key)?
    } else {
        let stored = integrations::value(d, s, field, key)?;
        fingerprint(
            &STANDARD
                .decode(stored.trim_ascii())
                .map_err(|_| "apple-file")?,
        )
    };
    if local != resource.fingerprint {
        return Err("apple-mismatch".into());
    }
    let evidence = Evidence {
        kind: resource.kind,
        id: resource.id,
        name: resource.name,
        checked: report.checked.clone(),
        state: resource.state,
    };
    let entry = d
        .entries
        .get_mut(&Document::address(s, field))
        .ok_or("missing")?;
    entry.expires = resource.expires;
    entry.apple_check = Some(evidence);
    d.event(s, field, "apple-apply");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openssl::{
        asn1::Asn1Time,
        bn::BigNum,
        ec::{EcGroup, EcKey},
        sign::Verifier,
        x509::X509NameBuilder,
    };
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        thread,
    };

    fn setup() -> (
        Document,
        Scope,
        Key,
        Credentials,
        PKey<openssl::pkey::Private>,
    ) {
        let key = Zeroizing::new(vault::random());
        let mut d = Document::initial();
        let s = Scope::default();
        let private = PKey::from_ec_key(
            EcKey::generate(&EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap()).unwrap(),
        )
        .unwrap();
        for (field, value) in [
            (
                "APPLE_API_ISSUER",
                b"01234567-89ab-4cde-8012-3456789abcde".to_vec(),
            ),
            ("APPLE_API_KEY", b"KEY1234567".to_vec()),
            (
                "APPLE_API_KEY_P8",
                private.private_key_to_pem_pkcs8().unwrap(),
            ),
        ] {
            d.save(&s, field, &value, "", &key).unwrap();
        }
        let auth = Credentials::load(&d, &s, &key).unwrap();
        (d, s, key, auth, private)
    }
    fn certificate(private: &PKey<openssl::pkey::Private>) -> X509 {
        let mut name = X509NameBuilder::new().unwrap();
        name.append_entry_by_text("CN", "Apple API test").unwrap();
        let name = name.build();
        let mut cert = X509::builder().unwrap();
        cert.set_version(2).unwrap();
        cert.set_subject_name(&name).unwrap();
        cert.set_issuer_name(&name).unwrap();
        cert.set_pubkey(private).unwrap();
        cert.set_not_before(&Asn1Time::days_from_now(0).unwrap())
            .unwrap();
        cert.set_not_after(&Asn1Time::days_from_now(30).unwrap())
            .unwrap();
        cert.sign(private, MessageDigest::sha256()).unwrap();
        cert.build()
    }
    fn server(replies: Vec<(u16, Value)>) -> (Api, thread::JoinHandle<Vec<(String, String)>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let api = Api::at(&format!("http://{}", listener.local_addr().unwrap()));
        let worker = thread::spawn(move || {
            let mut requests = vec![];
            for (status, data) in replies {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut reader = BufReader::new(&socket);
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let route = line.trim().to_owned();
                let mut token = String::new();
                loop {
                    line.clear();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    if let Some((name, value)) = line.split_once(':') {
                        if name.eq_ignore_ascii_case("authorization") {
                            token = value.trim().strip_prefix("Bearer ").unwrap().into();
                        }
                    }
                }
                requests.push((route, token));
                let body = data.to_string();
                write!(socket,"HTTP/1.1 {status} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            }
            requests
        });
        (api, worker)
    }
    #[test]
    fn scoped_jwt_is_es256_with_raw_signature_and_short_lifetime() {
        let (_, _, _, auth, private) = setup();
        let token = auth.token().unwrap();
        let parts: Vec<_> = token.split('.').collect();
        let header: Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[0]).unwrap()).unwrap();
        let claims: Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap();
        assert_eq!(header["alg"], "ES256");
        assert_eq!(header["kid"], "KEY1234567");
        assert_eq!(claims["aud"], "appstoreconnect-v1");
        assert_eq!(
            claims["scope"],
            json!([format!("GET {CERTS}"), format!("GET {PROFILES}")])
        );
        assert!(claims["exp"].as_i64().unwrap() - claims["iat"].as_i64().unwrap() <= 360);
        let signature = URL_SAFE_NO_PAD.decode(parts[2]).unwrap();
        assert_eq!(signature.len(), 64);
        let der = EcdsaSig::from_private_components(
            BigNum::from_slice(&signature[..32]).unwrap(),
            BigNum::from_slice(&signature[32..]).unwrap(),
        )
        .unwrap()
        .to_der()
        .unwrap();
        let mut verifier = Verifier::new(MessageDigest::sha256(), &private).unwrap();
        verifier
            .update(format!("{}.{}", parts[0], parts[1]).as_bytes())
            .unwrap();
        assert!(verifier.verify(&der).unwrap());
    }
    #[test]
    fn fetch_and_match_resources_never_apply_unrelated_expiry() {
        let (mut d, s, key, auth, private) = setup();
        let cert = certificate(&private);
        let der = cert.to_der().unwrap();
        let expiry = integrations::certificate(&der).unwrap().0;
        let profile = b"provisioning-profile-fixture";
        let (api, worker) = server(vec![
            (
                200,
                json!({"data":[{"id":"cert-1","type":"certificates","attributes":{"name":"Distribution","certificateType":"DISTRIBUTION","expirationDate":format!("{expiry}T00:00:00Z"),"certificateContent":STANDARD.encode(&der)}}],"links":{"next":null}}),
            ),
            (
                200,
                json!({"data":[{"id":"profile-1","type":"profiles","attributes":{"name":"Store profile","profileType":"MAC_APP_STORE","profileState":"INVALID","expirationDate":"2030-01-01T00:00:00Z","profileContent":STANDARD.encode(profile)}}],"links":{"next":null}}),
            ),
        ]);
        let report = fetch(&api, &auth, || Ok(())).unwrap();
        let requests = worker.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests
            .iter()
            .all(|r| r.0.starts_with("GET ") && !r.1.is_empty()));
        assert_eq!(report.resources.len(), 2);
        let id = report.id.clone();
        d.apple_reports.insert(Report::address(&s), report);
        let mut p12 = Pkcs12::builder();
        p12.name("fixture").pkey(&private).cert(&cert);
        d.save(
            &s,
            "APPLE_CERTIFICATE",
            STANDARD
                .encode(p12.build2("export-test").unwrap().to_der().unwrap())
                .as_bytes(),
            "",
            &key,
        )
        .unwrap();
        d.save(&s, "APPLE_CERTIFICATE_PASSWORD", b"export-test", "", &key)
            .unwrap();
        apply(
            &mut d,
            &s,
            "APPLE_CERTIFICATE",
            &id,
            "cert-1",
            "certificates",
            &key,
        )
        .unwrap();
        assert_eq!(d.entry(&s, "APPLE_CERTIFICATE").unwrap().expires, expiry);
        let store = Scope {
            target: "mac-store".into(),
            ..s.clone()
        };
        d.save(
            &store,
            "APPLE_PROVISIONING_PROFILE",
            STANDARD.encode(b"wrong-profile").as_bytes(),
            "",
            &key,
        )
        .unwrap();
        assert_eq!(
            apply(
                &mut d,
                &store,
                "APPLE_PROVISIONING_PROFILE",
                &id,
                "profile-1",
                "profiles",
                &key
            )
            .unwrap_err(),
            "apple-mismatch"
        );
        assert_eq!(
            d.entry(&store, "APPLE_PROVISIONING_PROFILE")
                .unwrap()
                .expires,
            ""
        );
        d.save(
            &store,
            "APPLE_PROVISIONING_PROFILE",
            STANDARD.encode(profile).as_bytes(),
            "",
            &key,
        )
        .unwrap();
        apply(
            &mut d,
            &store,
            "APPLE_PROVISIONING_PROFILE",
            &id,
            "profile-1",
            "profiles",
            &key,
        )
        .unwrap();
        assert_eq!(
            d.entry(&store, "APPLE_PROVISIONING_PROFILE")
                .unwrap()
                .apple_check
                .as_ref()
                .unwrap()
                .state,
            "INVALID"
        );
        assert!(apply(
            &mut d,
            &store,
            "APPLE_PROVISIONING_PROFILE",
            "old-report",
            "profile-1",
            "profiles",
            &key
        )
        .is_err());
        d.save(&s, "APPLE_API_KEY", b"OTHERKEY12", "", &key)
            .unwrap();
        assert_eq!(
            apply(
                &mut d,
                &store,
                "APPLE_PROVISIONING_PROFILE",
                &id,
                "profile-1",
                "profiles",
                &key
            )
            .unwrap_err(),
            "apple-stale"
        );
    }
    #[test]
    fn pagination_rejects_foreign_hosts_and_auth_errors_are_explicit() {
        let (_, _, _, auth, _) = setup();
        for (status, error) in [
            (401, "apple-auth"),
            (403, "apple-denied"),
            (429, "apple-rate"),
        ] {
            let (api, worker) = server(vec![(status, json!({}))]);
            assert_eq!(fetch(&api, &auth, || Ok(())).err().unwrap(), error);
            assert_eq!(worker.join().unwrap().len(), 1);
        }
        let (api, worker) = server(vec![(
            200,
            json!({"data":[],"links":{"next":"https://evil.example/v1/certificates"}}),
        )]);
        assert_eq!(
            fetch(&api, &auth, || Ok(())).err().unwrap(),
            "apple-response"
        );
        worker.join().unwrap();
        let (api, worker) = server(vec![
            (
                200,
                json!({"data":[],"links":{"next":format!("{ORIGIN}{CERTS}&cursor=next")}}),
            ),
            (200, json!({"data":[],"links":{"next":null}})),
            (200, json!({"data":[],"links":{"next":null}})),
        ]);
        assert!(fetch(&api, &auth, || Ok(())).unwrap().resources.is_empty());
        let requests = worker.join().unwrap();
        assert_eq!(requests.len(), 3);
        assert!(requests[1].0.contains("cursor=next"));
        assert!(requests.iter().all(|request| request.1 == requests[0].1));
        assert!(next_page(
            "https://api.appstoreconnect.apple.com/v1/certificates?cursor=next",
            "certificates"
        )
        .is_ok());
        assert!(next_page(
            "http://api.appstoreconnect.apple.com/v1/certificates",
            "certificates"
        )
        .is_err());
        assert!(next_page(
            "https://api.appstoreconnect.apple.com/v1/profiles",
            "certificates"
        )
        .is_err());
        assert_eq!(
            fetch(&Api::at("http://127.0.0.1:1"), &auth, || Err(
                "cancel".into()
            ))
            .err()
            .unwrap(),
            "cancel"
        );
    }
}
