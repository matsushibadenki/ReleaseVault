use super::*;
use openssl::{hash::MessageDigest, pkey::PKey, rsa::Rsa, sign::Verifier};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    thread,
};
use zeroize::Zeroizing;

struct Request {
    route: String,
    auth: String,
    body: Value,
}
fn server(responses: Vec<(u16, Value)>) -> (Api, thread::JoinHandle<Vec<Request>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let api = Api::fixture(&format!("http://{}", listener.local_addr().unwrap()));
    let handle = thread::spawn(move || {
        responses
            .into_iter()
            .map(|(status, data)| serve_request(&listener, status, data))
            .collect()
    });
    (api, handle)
}
fn serve_request(listener: &TcpListener, status: u16, data: Value) -> Request {
    let (mut socket, _) = listener.accept().unwrap();
    let mut reader = BufReader::new(&socket);
    let mut route = String::new();
    reader.read_line(&mut route).unwrap();
    let mut auth = String::new();
    let mut length = 0;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).unwrap();
        let Some((name, value)) = header.split_once(':') else {
            break;
        };
        match name.to_ascii_lowercase().as_str() {
            "authorization" => auth = value.trim().to_owned(),
            "content-length" => length = value.trim().parse().unwrap(),
            _ => {}
        }
    }
    let mut raw = vec![0; length];
    reader.read_exact(&mut raw).unwrap();
    let body = if raw.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&raw).unwrap()
    };
    let payload = if data.is_null() {
        String::new()
    } else {
        data.to_string()
    };
    write!(socket,"HTTP/1.1 {status} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",payload.len()).unwrap();
    Request {
        route: route.trim_end_matches(" HTTP/1.1\r\n").to_owned(),
        auth,
        body,
    }
}
fn setup() -> (Document, Scope, Key, Vec<u8>) {
    let key = Zeroizing::new(vault::random());
    let pem = Rsa::generate(2048).unwrap().private_key_to_pem().unwrap();
    let scope = Scope::default();
    let mut doc = Document::initial();
    doc.github_apps.insert(
        scope.project,
        Connection {
            app: 42,
            installation: 100,
            pem: vault::seal(&key, &pem).unwrap(),
            account: "studio".into(),
            organization: true,
            limits: Grants {
                secrets: Level::Write,
                variables: Level::Write,
                environments: Level::Write,
            },
            repos: vec![Repo {
                id: 17,
                full_name: "studio/mobile".into(),
            }],
            policy: BTreeMap::new(),
        },
    );
    (doc, scope, key, pem)
}
fn verify_jwt(auth: &str, pem: &[u8]) {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let (payload, sig) = auth
        .strip_prefix("Bearer ")
        .unwrap()
        .rsplit_once('.')
        .unwrap();
    let key = PKey::private_key_from_pem(pem).unwrap();
    let mut v = Verifier::new(MessageDigest::sha256(), &key).unwrap();
    v.update(payload.as_bytes()).unwrap();
    assert!(v.verify(&URL_SAFE_NO_PAD.decode(sig).unwrap()).unwrap());
    let claims: Value = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(payload.split_once('.').unwrap().1)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(claims["iss"], "42");
    assert!(claims["exp"].as_i64().unwrap() > chrono::Utc::now().timestamp());
}
const NAME: &str = "studio/mobile";
fn dest(secret: bool, env: &str) -> Destination<'_> {
    Destination {
        id: 17,
        name: NAME,
        env,
        secret,
    }
}
fn token() -> (u16, Value) {
    (201, json!({"token":"fixture-token"}))
}
#[test]
fn discovery_checks_identity_paginates_and_reduces_grants() {
    let (mut d, s, _, pem) = setup();
    let c = d.github_apps.get_mut(&s.project).unwrap();
    c.configure(17, c.limits.clone()).unwrap();
    c.policy.insert(12345, c.limits.clone());
    let mut page: Vec<_> = (1..=100)
        .map(|n| json!({"id":n,"full_name":format!("studio/repo{n}")}))
        .collect();
    page[16] = json!({"id":17,"full_name":NAME});
    let info = json!({"id":100,"app_id":42,"account":{"login":"studio"},"target_type":"Organization","permissions":{"secrets":"read","metadata":"read","actions_variables":"write"}});
    let (api, log) = server(vec![
        (200, info),
        token(),
        (200, json!({"repositories":page})),
        (
            200,
            json!({"repositories":[{"id":101,"full_name":"studio/.github"}]}),
        ),
    ]);
    discover(&api, c, &pem).unwrap();
    let req = log.join().unwrap();
    verify_jwt(&req[0].auth, &pem);
    assert_eq!(req[1].body, json!({"permissions":{"metadata":"read"}}));
    assert!(req[3].route.ends_with("page=2"));
    assert_eq!(req[3].auth, "Bearer fixture-token");
    assert_eq!(c.repos.len(), 101);
    assert_eq!(c.policy.len(), 1);
    assert!(c
        .permission(17, NAME, "actions_variables", Level::Write)
        .is_ok());
    assert_eq!(c.view()["limits"]["variables"], "write");
    assert!(c.permission(17, NAME, "secrets", Level::Read).is_ok());
    assert!(c.permission(17, NAME, "secrets", Level::Write).is_err());
    assert!(c
        .permission(101, "studio/.github", "secrets", Level::Read)
        .is_err());
    assert!(c.view().get("pem").is_none());
}
#[test]
fn env_secret_is_sealed_and_poll_requests_read_only() {
    let (mut d, s, key, pem) = setup();
    let k = "APPLE_CERTIFICATE_PASSWORD";
    d.save(&s, k, b"test-value", "", &key).unwrap();
    let c = d.github_apps.get_mut(&s.project).unwrap();
    c.configure(
        17,
        Grants {
            environments: Level::Write,
            ..Default::default()
        },
    )
    .unwrap();
    let sk = crypto_box::SecretKey::generate(&mut rand::rngs::OsRng);
    let (api, log) = server(vec![
        token(),
        (
            200,
            json!({"key_id":"pk","key":STANDARD.encode(sk.public_key().as_bytes())}),
        ),
        (204, Value::Null),
        token(),
        (200, json!({"updated_at":"2026-01-01T00:00:00Z"})),
    ]);
    send_with(&api, &mut d, &s, k, dest(true, "QA/beta"), &key).unwrap();
    let b = d.local_entry(&s, k).unwrap().binding.as_ref().unwrap();
    assert_eq!(
        poll_with(&api, &d, &s, k, &key, b).unwrap(),
        "2026-01-01T00:00:00Z"
    );
    let log = log.join().unwrap();
    verify_jwt(&log[0].auth, &pem);
    assert_eq!(
        log[0].body,
        json!({"repository_ids":[17],"permissions":{"environments":"write"}})
    );
    assert_eq!(
        log[1].route,
        "GET /repos/studio/mobile/environments/QA%2Fbeta/secrets/public-key"
    );
    assert_eq!(log[2].auth, "Bearer fixture-token");
    assert_eq!(
        sk.unseal(
            &STANDARD
                .decode(log[2].body["encrypted_value"].as_str().unwrap())
                .unwrap()
        )
        .unwrap(),
        b"test-value"
    );
    assert_eq!(
        log[3].body,
        json!({"repository_ids":[17],"permissions":{"environments":"read"}})
    );
    disconnect(&mut d, &s).unwrap();
    assert_eq!(
        integrations::fetch_remote(&mut d, &s, k, &key).unwrap_err(),
        "app-config"
    );
}
#[test]
fn variables_create_on_missing_and_fail_closed() {
    let (mut d, s, key, _) = setup();
    let k = "APPLE_TEAM_ID";
    d.save(&s, k, b"fixture", "", &key).unwrap();
    let (api, log) = server(vec![
        token(),
        (403, Value::Null),
        token(),
        (404, Value::Null),
        (201, Value::Null),
    ]);
    assert_eq!(
        send_with(&api, &mut d, &s, k, dest(false, ""), &key).unwrap_err(),
        "app-policy"
    );
    let c = d.github_apps.get_mut(&s.project).unwrap();
    c.configure(
        17,
        Grants {
            variables: Level::Write,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(c
        .permission(18, NAME, "actions_variables", Level::Write)
        .is_err());
    assert!(c
        .permission(17, "studio/other", "actions_variables", Level::Write)
        .is_err());
    assert_eq!(
        integrations::github(&mut d, &s, k, NAME, "", &key).unwrap_err(),
        "app-policy"
    );
    assert_eq!(
        send_with(&api, &mut d, &s, k, dest(false, ""), &key).unwrap_err(),
        "app-denied"
    );
    assert!(d.local_entry(&s, k).unwrap().binding.is_none());
    send_with(&api, &mut d, &s, k, dest(false, ""), &key).unwrap();
    let log = log.join().unwrap();
    assert_eq!(
        log[0].body,
        json!({"repository_ids":[17],"permissions":{"actions_variables":"write"}})
    );
    assert_eq!(
        log[1].route,
        "PATCH /repos/studio/mobile/actions/variables/APPLE_TEAM_ID"
    );
    assert_eq!(log[4].route, "POST /repos/studio/mobile/actions/variables");
    assert_eq!(log[4].body, json!({"name":k,"value":"fixture"}));
}

#[test]
fn read_only_repository_can_bind_without_sending() {
    let (mut d, s, key, _) = setup();
    let k = "APPLE_TEAM_ID";
    d.save(&s, k, b"local", "", &key).unwrap();
    let c = d.github_apps.get_mut(&s.project).unwrap();
    c.configure(
        17,
        Grants {
            variables: Level::Read,
            ..Default::default()
        },
    )
    .unwrap();
    let b = c.binding(NAME.into(), String::new(), 17);
    let (api, requests) = server(vec![
        token(),
        (
            200,
            json!({"updated_at":"2026-01-01T00:00:00Z","value":"remote-only"}),
        ),
    ]);
    bind_with(&api, &mut d, &s, k, &key, b).unwrap();
    let requests = requests.join().unwrap();
    assert_eq!(
        requests[0].body,
        json!({"repository_ids":[17],"permissions":{"actions_variables":"read"}})
    );
    assert_eq!(
        requests[1].route,
        "GET /repos/studio/mobile/actions/variables/APPLE_TEAM_ID"
    );
    assert!(d.local_entry(&s, k).unwrap().synced.is_empty());
    assert!(!serde_json::to_string(&d).unwrap().contains("remote-only"));
}
