use thiserror::Error;

#[derive(Debug, Error, Clone, Copy)]
pub enum Error {
    #[error("Failure verifying MAC")]
    Decrypt,
    #[error("Invalid tls certificate ")]
    TLSCert,
}

impl From<aes_gcm_siv::Error> for Error {
    fn from(_: aes_gcm_siv::Error) -> Self {
        Self::Decrypt
    }
}

impl From<rcgen::Error> for Error {
    fn from(_: rcgen::Error) -> Self {
        Self::TLSCert
    }
}
