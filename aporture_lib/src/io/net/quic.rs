#![allow(clippy::module_name_repetitions)]

use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;

use quinn::crypto::rustls::QuicClientConfig;
use quinn::{ClientConfig, ServerConfig, TokioRuntime};
use quinn::{Connection, Endpoint, EndpointConfig, RecvStream, SendStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinHandle;

use crate::crypto::cert::Certificate;
use crate::crypto::cipher::Cipher;
use crate::net::peer::{Encryptable, Peer};

#[derive(Debug)]
pub struct QuicConnection {
    connection_address: SocketAddr,
    cipher: Arc<Cipher>,
    endpoint: Endpoint,
    connection: Connection,
    keepalive_handle: JoinHandle<()>,
    kind: Kind,
}

#[derive(Debug)]
enum Kind {
    Server,
    Client,
}

#[derive(Debug)]
pub struct QuicNetworkPeer {
    cipher: Arc<Cipher>,
    sender: SendStream,
    receiver: RecvStream,
}

// TODO: USE REAL CERTIFIATES
impl QuicConnection {
    pub async fn client(
        server_address: SocketAddr,
        socket: UdpSocket,
        cipher: Arc<Cipher>,
        keepalive_handle: JoinHandle<()>,
    ) -> Result<Self, crate::io::Error> {
        let config = ClientConfig::new(Arc::new(
            QuicClientConfig::try_from(
                quinn::rustls::ClientConfig::builder()
                    .dangerous()
                    .with_custom_certificate_verifier(remove::SkipServerVerification::new())
                    .with_no_client_auth(),
            )
            .expect("Valid quinn client configuration"),
        ));

        let mut endpoint = Endpoint::new(
            EndpointConfig::default(),
            None,
            socket,
            Arc::new(TokioRuntime),
        )?;
        endpoint.set_default_client_config(config);

        let connection = endpoint
            .connect(server_address, "localhost")
            .expect("Valid quinn endpoint configuration")
            .await?;

        Ok(Self {
            connection_address: server_address,
            cipher,
            endpoint,
            connection,
            keepalive_handle,
            kind: Kind::Client,
        })
    }

    pub async fn server(
        connection_address: SocketAddr,
        socket: UdpSocket,
        cipher: Arc<Cipher>,
        keepalive_handle: JoinHandle<()>,
    ) -> Result<Self, crate::io::Error> {
        let self_signed = Certificate::default();

        let config = ServerConfig::with_single_cert(vec![self_signed.cert], self_signed.key)
            .expect("Valid quinn server configuration");

        let endpoint = Endpoint::new(
            EndpointConfig::default(),
            Some(config),
            socket,
            Arc::new(TokioRuntime),
        )?;

        let connection = endpoint
            .accept()
            .await
            .expect("Valid quinn endpoint configuration")
            .await?;

        Ok(Self {
            connection_address,
            cipher,
            endpoint,
            connection,
            keepalive_handle,
            kind: Kind::Server,
        })
    }

    pub async fn finish(self) {
        match self.kind {
            Kind::Server => {
                self.connection.closed().await;
            }
            Kind::Client => self.connection.close(0u16.into(), &[]),
        }

        self.keepalive_handle.abort();

        self.endpoint.wait_idle().await;
    }

    pub async fn new_stream(&mut self) -> Result<QuicNetworkPeer, std::io::Error> {
        let (sender, receiver) = match self.kind {
            Kind::Server => self.connection.accept_bi().await?,
            Kind::Client => self.connection.open_bi().await?,
        };

        Ok(QuicNetworkPeer {
            cipher: Arc::clone(&self.cipher),
            sender,
            receiver,
        })
    }

    #[must_use]
    pub const fn address(&self) -> SocketAddr {
        self.connection_address
    }
}

impl Peer for QuicNetworkPeer {
    fn writer(&mut self) -> impl AsyncWriteExt + Unpin {
        &mut self.sender
    }

    fn reader(&mut self) -> impl AsyncReadExt + Unpin {
        &mut self.receiver
    }
}

impl Encryptable for QuicNetworkPeer {
    fn cipher(&self) -> impl AsRef<Cipher> {
        &self.cipher
    }
}

mod remove {
    use std::sync::Arc;

    use quinn::rustls;
    use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
    use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
    use rustls::{DigitallySignedStruct, SignatureScheme};

    #[derive(Debug)]
    pub struct SkipServerVerification(Arc<rustls::crypto::CryptoProvider>);

    impl SkipServerVerification {
        pub fn new() -> Arc<Self> {
            Arc::new(Self(Arc::new(rustls::crypto::ring::default_provider())))
        }
    }

    impl ServerCertVerifier for SkipServerVerification {
        fn verify_server_cert(
            &self,
            _end_entity: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _server_name: &ServerName<'_>,
            _ocsp_response: &[u8],
            _now: UnixTime,
        ) -> Result<ServerCertVerified, rustls::Error> {
            Ok(ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            message: &[u8],
            cert: &CertificateDer<'_>,
            dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, quinn::rustls::Error> {
            rustls::crypto::verify_tls12_signature(
                message,
                cert,
                dss,
                &self.0.signature_verification_algorithms,
            )
        }

        fn verify_tls13_signature(
            &self,
            message: &[u8],
            cert: &CertificateDer<'_>,
            dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, quinn::rustls::Error> {
            rustls::crypto::verify_tls13_signature(
                message,
                cert,
                dss,
                &self.0.signature_verification_algorithms,
            )
        }

        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            self.0.signature_verification_algorithms.supported_schemes()
        }
    }
}
