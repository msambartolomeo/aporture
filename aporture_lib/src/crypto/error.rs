use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Failure verifying MAC")]
    Decrypt,
}

impl From<aes_gcm_siv::Error> for Error {
    fn from(_: aes_gcm_siv::Error) -> Self {
        Self::Decrypt
    }
}
