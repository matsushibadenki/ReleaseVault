//! Match a separately selected certificate against a private key already in the vault.
use crate::{
    integrations,
    model::{Document, Scope},
    vault::{Key, R},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use openssl::{
    hash::MessageDigest,
    pkcs12::Pkcs12,
    pkey::{PKey, Private},
    x509::X509,
};
use zeroize::Zeroizing;

fn private_key(stored: &[u8], passphrase: &str) -> R<PKey<Private>> {
    if passphrase.contains('\0') {
        return Err("key-format".into());
    }
    let trimmed = stored.trim_ascii();
    let decoded;
    let bytes = if trimmed.starts_with(b"-----BEGIN ") {
        trimmed
    } else {
        decoded = Zeroizing::new(STANDARD.decode(trimmed).map_err(|_| "key-format")?);
        decoded.as_slice()
    };
    if bytes.starts_with(b"-----BEGIN ") {
        return PKey::private_key_from_pem_passphrase(bytes, passphrase.as_bytes())
            .map_err(|_| "key-format".into());
    }
    if let Ok(container) = Pkcs12::from_der(bytes) {
        return container
            .parse2(passphrase)
            .map_err(|_| "key-format")?
            .pkey
            .ok_or_else(|| "key-format".into());
    }
    PKey::private_key_from_der(bytes)
        .or_else(|_| PKey::private_key_from_pkcs8_passphrase(bytes, passphrase.as_bytes()))
        .map_err(|_| "key-format".into())
}

pub fn match_certificate(
    d: &mut Document,
    s: &Scope,
    k: &str,
    certificate: &[u8],
    passphrase: &str,
    key: &Key,
) -> R<()> {
    d.require_owner(s, k)?;
    if d.def(s, k)?.kind != "secret" {
        return Err("key-format".into());
    }
    let private = private_key(&integrations::value(d, s, k, key)?, passphrase)?;
    let cert = X509::from_der(certificate)
        .or_else(|_| X509::from_pem(certificate))
        .map_err(|_| "format")?;
    let public = cert.public_key().map_err(|_| "format")?;
    if !private.public_eq(&public) {
        return Err("key-mismatch".into());
    }
    let der = cert.to_der().map_err(|_| "format")?;
    let (expiry, subject) = integrations::certificate(&der)?;
    let fingerprint = cert.digest(MessageDigest::sha256()).map_err(|_| "crypto")?;
    let entry = d
        .entries
        .get_mut(&Document::address(s, k))
        .ok_or("missing")?;
    entry.expires = expiry;
    entry.evidence = subject;
    entry.matched_fingerprint = fingerprint
        .iter()
        .map(|v| format!("{v:02X}"))
        .collect::<Vec<_>>()
        .join(":");
    d.event(s, k, "key-match");
    Ok(())
}
