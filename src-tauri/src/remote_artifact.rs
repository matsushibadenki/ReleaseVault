// Stream public artifacts directly into signature verification. No vault keys or tokens go on the network.
use crate::{
    integrations,
    publishing::{RemoteCheck, VerifiedPlatform},
    vault::R,
};
use std::{
    io::{self, Read},
    time::{Duration, Instant},
};

const MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(180);

fn https_url(raw: &str) -> R<tauri::Url> {
    let url = tauri::Url::parse(raw).map_err(|_| "download-url")?;
    if url.scheme() != "https"
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err("download-url".into());
    }
    Ok(url)
}

struct LimitedReader<'a> {
    inner: Box<dyn Read + Send + Sync>,
    bytes: u64,
    started: Instant,
    cancelled: &'a dyn Fn() -> bool,
    failure: Option<&'static str>,
}
impl Read for LimitedReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let failure = if (self.cancelled)() {
            Some("cancel")
        } else if self.started.elapsed() > DEADLINE {
            Some("download-timeout")
        } else {
            None
        };
        if let Some(error) = failure {
            self.failure = Some(error);
            return Err(io::Error::other(error));
        }
        let n = self.inner.read(buf)?;
        self.bytes += n as u64;
        if self.bytes > MAX_BYTES {
            self.failure = Some("download-size");
            return Err(io::Error::other("download-size"));
        }
        Ok(n)
    }
}

pub fn verify(
    entry: &VerifiedPlatform,
    version: &str,
    cancelled: impl Fn() -> bool,
) -> RemoteCheck {
    let mut bytes = None;
    let result = fetch(entry, version, &cancelled).map(|size| {
        bytes = Some(size);
    });
    RemoteCheck {
        checked: chrono::Utc::now().to_rfc3339(),
        status: result.err().unwrap_or_else(|| "verified".into()),
        bytes,
    }
}
fn fetch(entry: &VerifiedPlatform, version: &str, cancelled: &dyn Fn() -> bool) -> R<u64> {
    let agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(15))
        .timeout_write(Duration::from_secs(15))
        .build();
    fetch_with_agent(entry, version, cancelled, &agent)
}
fn fetch_with_agent(
    entry: &VerifiedPlatform,
    version: &str,
    cancelled: &dyn Fn() -> bool,
    agent: &ureq::Agent,
) -> R<u64> {
    let started = Instant::now();
    let mut url = https_url(&entry.url)?;
    for _ in 0..=5 {
        if cancelled() {
            return Err("cancel".into());
        }
        if started.elapsed() > DEADLINE {
            return Err("download-timeout".into());
        }
        let response = match agent
            .get(url.as_str())
            .set("User-Agent", "ReleaseVault")
            .set("Accept-Encoding", "identity")
            .call()
        {
            Ok(response) => response,
            Err(ureq::Error::Status(404, _)) => return Err("download-missing".into()),
            Err(ureq::Error::Status(401 | 403, _)) => return Err("download-denied".into()),
            Err(_) => return Err("download".into()),
        };
        if matches!(response.status(), 301 | 302 | 303 | 307 | 308) {
            let location = response.header("Location").ok_or("download-url")?;
            url = https_url(url.join(location).map_err(|_| "download-url")?.as_str())?;
            continue;
        }
        if response.status() != 200 {
            return Err("download".into());
        }
        if response
            .header("Content-Length")
            .and_then(|v| v.parse::<u64>().ok())
            .is_some_and(|size| size > MAX_BYTES)
        {
            return Err("download-size".into());
        }
        let mut reader = LimitedReader {
            inner: response.into_reader(),
            bytes: 0,
            started,
            cancelled,
            failure: None,
        };
        let verified =
            integrations::verify_reader(&entry.public_key, entry.signature.as_bytes(), &mut reader);
        if let Some(error) = reader.failure {
            return Err(error.into());
        }
        if verified?.as_deref() != Some(version) {
            return Err("version".into());
        }
        return Ok(reader.bytes);
    }
    Err("download-redirects".into())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use openssl::{
        asn1::Asn1Time,
        hash::MessageDigest,
        pkey::PKey,
        rsa::Rsa,
        ssl::{SslAcceptor, SslMethod},
        x509::{
            extension::{BasicConstraints, SubjectAlternativeName},
            X509NameBuilder, X509,
        },
    };
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        sync::Arc,
        thread,
    };

    // Real TLS, including a trusted fixture CA. Production TLS configuration is unchanged.
    fn fixture(
        responses: Vec<(u16, Vec<(&'static str, String)>, Vec<u8>)>,
    ) -> (ureq::Agent, String, thread::JoinHandle<()>) {
        let key = PKey::from_rsa(Rsa::generate(2048).unwrap()).unwrap();
        let mut name = X509NameBuilder::new().unwrap();
        name.append_entry_by_text("CN", "localhost").unwrap();
        let name = name.build();
        let mut root = X509::builder().unwrap();
        root.set_version(2).unwrap();
        root.set_subject_name(&name).unwrap();
        root.set_issuer_name(&name).unwrap();
        root.set_pubkey(&key).unwrap();
        root.set_not_before(&Asn1Time::days_from_now(0).unwrap())
            .unwrap();
        root.set_not_after(&Asn1Time::days_from_now(1).unwrap())
            .unwrap();
        root.append_extension(BasicConstraints::new().critical().ca().build().unwrap())
            .unwrap();
        root.sign(&key, MessageDigest::sha256()).unwrap();
        let root = root.build();
        let mut leaf = X509::builder().unwrap();
        leaf.set_version(2).unwrap();
        leaf.set_subject_name(&name).unwrap();
        leaf.set_issuer_name(&name).unwrap();
        leaf.set_pubkey(&key).unwrap();
        leaf.set_not_before(&Asn1Time::days_from_now(0).unwrap())
            .unwrap();
        leaf.set_not_after(&Asn1Time::days_from_now(1).unwrap())
            .unwrap();
        let san = SubjectAlternativeName::new()
            .ip("127.0.0.1")
            .build(&leaf.x509v3_context(Some(&root), None))
            .unwrap();
        leaf.append_extension(san).unwrap();
        leaf.sign(&key, MessageDigest::sha256()).unwrap();
        let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
        acceptor.set_private_key(&key).unwrap();
        acceptor.set_certificate(&leaf.build()).unwrap();
        let acceptor = acceptor.build();
        let mut roots = rustls::RootCertStore::empty();
        roots
            .add(rustls::pki_types::CertificateDer::from(
                root.to_der().unwrap(),
            ))
            .unwrap();
        let tls = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        let agent = ureq::AgentBuilder::new()
            .redirects(0)
            .timeout(Duration::from_secs(5))
            .tls_config(Arc::new(tls))
            .build();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("https://{}/artifact", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            for (status, headers, body) in responses {
                let (socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut socket = acceptor.accept(socket).unwrap();
                {
                    let mut reader = BufReader::new(&mut socket);
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    assert!(line.starts_with("GET "));
                    loop {
                        line.clear();
                        reader.read_line(&mut line).unwrap();
                        if line == "\r\n" {
                            break;
                        }
                    }
                }
                write!(socket, "HTTP/1.1 {status} Result\r\nConnection: close\r\n").unwrap();
                if !headers.iter().any(|(h, _)| *h == "Content-Length") {
                    write!(socket, "Content-Length: {}\r\n", body.len()).unwrap();
                }
                for (header, value) in headers {
                    write!(socket, "{header}: {value}\r\n").unwrap();
                }
                socket.write_all(b"\r\n").unwrap();
                socket.write_all(&body).unwrap();
            }
        });
        (agent, url, handle)
    }

    pub fn signed_https_cases(entry: &VerifiedPlatform, body: &[u8]) {
        let cases = [
            (
                vec![
                    (302, vec![("Location", "/download".into())], vec![]),
                    (200, vec![], body.to_vec()),
                ],
                None,
            ),
            (
                vec![(200, vec![], b"tampered artifact".to_vec())],
                Some("mismatch"),
            ),
            (vec![(404, vec![], vec![])], Some("download-missing")),
            (
                vec![(
                    200,
                    vec![("Content-Length", (MAX_BYTES + 1).to_string())],
                    vec![],
                )],
                Some("download-size"),
            ),
            (
                vec![(
                    302,
                    vec![("Location", "http://example.com/unsafe".into())],
                    vec![],
                )],
                Some("download-url"),
            ),
        ];
        for (responses, error) in cases {
            let (agent, url, server) = fixture(responses);
            let mut entry = entry.clone();
            entry.url = url;
            let result = fetch_with_agent(&entry, "1.0.0", &|| false, &agent);
            server.join().unwrap();
            match error {
                Some(error) => assert_eq!(result.unwrap_err(), error),
                None => assert_eq!(result.unwrap(), body.len() as u64),
            }
        }
    }

    #[test]
    fn streaming_limits_deadline_cancellation_and_url_validation() {
        for url in [
            "http://example.com/a",
            "https://user:pass@example.com/a",
            "https://example.com/a#frag",
            "file:///tmp/a",
        ] {
            assert!(https_url(url).is_err());
        }
        let mut reader = LimitedReader {
            inner: Box::new(io::Cursor::new(vec![1])),
            bytes: MAX_BYTES,
            started: Instant::now(),
            cancelled: &|| false,
            failure: None,
        };
        assert!(reader.read(&mut [0]).is_err());
        assert_eq!(reader.failure, Some("download-size"));
        let mut reader = LimitedReader {
            inner: Box::new(io::Cursor::new(vec![1])),
            bytes: 0,
            started: Instant::now() - DEADLINE - Duration::from_secs(1),
            cancelled: &|| false,
            failure: None,
        };
        assert!(reader.read(&mut [0]).is_err());
        assert_eq!(reader.failure, Some("download-timeout"));
        let mut reader = LimitedReader {
            inner: Box::new(io::Cursor::new(vec![1])),
            bytes: 0,
            started: Instant::now(),
            cancelled: &|| true,
            failure: None,
        };
        assert!(reader.read(&mut [0]).is_err());
        assert_eq!(reader.failure, Some("cancel"));
    }
}
