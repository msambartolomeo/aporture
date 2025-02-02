#![allow(clippy::module_name_repetitions)]

use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::Duration;

use quinn::crypto::rustls::QuicClientConfig;
use quinn::{ClientConfig, ServerConfig, TokioRuntime, TransportConfig};
use quinn::{Connection, Endpoint, EndpointConfig, RecvStream, SendStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinHandle;

use crate::crypto::cert::{Certificate, CertificateKey};
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

impl QuicConnection {
    pub async fn client(
        server_address: SocketAddr,
        socket: UdpSocket,
        cipher: Arc<Cipher>,
        certificate: Certificate,
        keepalive_handle: JoinHandle<()>,
    ) -> Result<Self, crate::io::Error> {
        let mut config = ClientConfig::new(Arc::new(
            QuicClientConfig::try_from(
                quinn::rustls::ClientConfig::builder()
                    .with_root_certificates(certificate.0)
                    .with_no_client_auth(),
            )
            .expect("Valid quinn client configuration"),
        ));
        config.transport_config(Self::transport_config());

        let mut endpoint = Endpoint::new(
            EndpointConfig::default(),
            None,
            socket,
            Arc::new(TokioRuntime),
        )
        .inspect_err(|_| keepalive_handle.abort())?;
        endpoint.set_default_client_config(config);

        let mut connection = Err(quinn::ConnectionError::TimedOut);

        for _ in 0..2 {
            log::info!("Trying connection");

            if let Ok(c) = tokio::time::timeout(
                Duration::from_secs(2),
                endpoint
                    .connect(server_address, &server_address.ip().to_string())
                    .expect("Valid quinn endpoint configuration"),
            )
            .await
            {
                connection = c;

                if connection.is_ok() {
                    break;
                }
            }
        }

        let connection = connection.inspect_err(|_| keepalive_handle.abort())?;

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
        certificate: CertificateKey,
        keepalive_handle: JoinHandle<()>,
    ) -> Result<Self, crate::io::Error> {
        let mut config = ServerConfig::with_single_cert(vec![certificate.cert], certificate.key)
            .expect("Valid quinn server configuration");
        config.transport_config(Self::transport_config());

        let endpoint = Endpoint::new(
            EndpointConfig::default(),
            Some(config),
            socket,
            Arc::new(TokioRuntime),
        )
        .inspect_err(|_| keepalive_handle.abort())?;

        let connection = endpoint
            .accept()
            .await
            .expect("Valid quinn endpoint configuration")
            .await
            .inspect_err(|_| keepalive_handle.abort())?;

        Ok(Self {
            connection_address,
            cipher,
            endpoint,
            connection,
            keepalive_handle,
            kind: Kind::Server,
        })
    }

    fn transport_config() -> Arc<TransportConfig> {
        let transport_config = TransportConfig::default();
        // NOTE: Keep default timeout
        // transport_config.max_idle_timeout(None);
        Arc::new(transport_config)
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

    pub async fn new_stream(&self) -> Result<QuicNetworkPeer, std::io::Error> {
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
