//! TLS for the relay connection and its URLs (`wss://`, `https://`):
//! rustls with the `ring` provider, verifying the relay's certificate
//! against the platform's trust store, which `rustls-native-certs` reads
//! once per process. The SDK's own tests also trust the test CA in
//! `tests/tls/`, and only that one besides the platform's store.

use std::sync::{Arc, OnceLock};

#[cfg(test)]
use rustls::pki_types::CertificateDer;
use rustls::{ClientConfig, RootCertStore};

/// The client configuration every TLS connection of the SDK uses, or none
/// when rustls cannot build one.
pub(crate) fn config() -> Option<Arc<ClientConfig>> {
    static CONFIG: OnceLock<Option<Arc<ClientConfig>>> = OnceLock::new();
    CONFIG.get_or_init(build).clone()
}

fn build() -> Option<Arc<ClientConfig>> {
    let mut roots = RootCertStore::empty();
    // A certificate the store cannot read is skipped; the others stay.
    let native = rustls_native_certs::load_native_certs();
    roots.add_parsable_certificates(native.certs);
    #[cfg(test)]
    roots
        .add(CertificateDer::from(
            include_bytes!("tests/tls/ca.der").to_vec(),
        ))
        .ok()?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .ok()?
        .with_root_certificates(roots)
        .with_no_client_auth();
    Some(Arc::new(config))
}
