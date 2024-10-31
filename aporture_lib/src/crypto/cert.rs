use std::sync::Arc;

use quinn::rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use quinn::rustls::RootCertStore;

#[derive(Debug)]
pub struct CertificateKey {
    pub cert: CertificateDer<'static>,
    pub key: PrivateKeyDer<'static>,
}

impl Clone for CertificateKey {
    fn clone(&self) -> Self {
        Self {
            cert: self.cert.clone(),
            key: self.key.clone_key(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Certificate(pub Arc<RootCertStore>);

impl Default for CertificateKey {
    fn default() -> Self {
        Self::new(vec!["localhost".into()]).expect("Valid certificacte generation")
    }
}

impl From<Vec<u8>> for Certificate {
    fn from(value: Vec<u8>) -> Self {
        let mut store = RootCertStore::empty();

        store.add(value.into()).expect("Certificate is valid");

        Self(Arc::new(store))
    }
}

impl CertificateKey {
    pub fn new(domains: Vec<String>) -> Result<Self, super::Error> {
        let self_signed = rcgen::generate_simple_self_signed(domains)?;

        let cert = self_signed.cert.into();
        let key = PrivatePkcs8KeyDer::from(self_signed.key_pair.serialize_der()).into();

        Ok(Self { cert, key })
    }

    pub fn cert_der(&self) -> Vec<u8> {
        self.cert.to_vec()
    }
}
