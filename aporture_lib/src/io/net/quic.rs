use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;

use quinn::crypto::rustls::QuicClientConfig;
use quinn::rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use quinn::{
    ClientConfig, Connection, Endpoint, EndpointConfig, RecvStream, SendStream, ServerConfig,
    TokioRuntime,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::crypto::cipher::Cipher;
use crate::net::peer::{Encryptable, Peer};

#[derive(Debug)]
pub struct QuicConnection {
    cipher: Arc<Cipher>,
    connection: Connection,
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
        socket: UdpSocket,
        cipher: Arc<Cipher>,
        server_address: SocketAddr,
    ) -> Option<Self> {
        let config = ClientConfig::new(Arc::new(
            QuicClientConfig::try_from(
                quinn::rustls::ClientConfig::builder()
                    .dangerous()
                    .with_custom_certificate_verifier(remove::SkipServerVerification::new())
                    .with_no_client_auth(),
            )
            .ok()?,
        ));

        let mut endpoint = Endpoint::new(
            EndpointConfig::default(),
            None,
            socket,
            Arc::new(TokioRuntime),
        )
        .ok()?;
        endpoint.set_default_client_config(config);

        let connection = endpoint
            .connect(server_address, "localhost")
            .ok()?
            .await
            .ok()?;

        Some(Self {
            cipher,
            connection,
            kind: Kind::Client,
        })
    }

    pub async fn server(
        socket: UdpSocket,
        cipher: Arc<Cipher>,
        client_address: SocketAddr,
    ) -> Option<Self> {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).ok()?;
        let cert_der = CertificateDer::from(cert.cert);
        let priv_key = PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der());

        let config =
            ServerConfig::with_single_cert(vec![cert_der.clone()], priv_key.into()).ok()?;

        let endpoint = Endpoint::new(
            EndpointConfig::default(),
            Some(config),
            socket,
            Arc::new(TokioRuntime),
        )
        .ok()?;

        let connection = endpoint.accept().await?.await.ok()?;

        (connection.remote_address() == client_address).then_some(Self {
            cipher,
            connection,
            kind: Kind::Server,
        })
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
