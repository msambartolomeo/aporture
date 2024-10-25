use quinn::rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

pub struct Certificate {
    pub cert: CertificateDer<'static>,
    pub key: PrivateKeyDer<'static>,
}

impl Default for Certificate {
    fn default() -> Self {
        Self::new(vec!["localhost".into()]).expect("Valid certificacte generation")
    }
}

impl Certificate {
    pub fn new(domains: Vec<String>) -> Result<Self, super::Error> {
        let self_signed = rcgen::generate_simple_self_signed(domains)?;

        let cert = self_signed.cert.into();
        let key = PrivatePkcs8KeyDer::from(self_signed.key_pair.serialize_der()).into();

        Ok(Self { cert, key })
    }
}
