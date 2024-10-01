use std::marker::PhantomData;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;

use spake2::{Ed25519Group, Identity, Password, Spake2};
use tokio::net::TcpStream;

use crate::crypto::cipher::Cipher;
use crate::crypto::hasher::Hasher;
use crate::crypto::Key;
use crate::fs::config::Config;
use crate::net::{EncryptedNetworkPeer, NetworkPeer};
use crate::parser::{EncryptedSerdeIO, SerdeIO};
use crate::protocol::{
    Hello, KeyExchangePayload, NegotiationPayload, PairKind, PairingResponseCode,
};

mod upnp;

pub mod error;
pub use error::Error;

const DEFAULT_RECEIVER_PORT: u16 = 8082;

pub struct AporturePairingProtocolState {
    protocol_version: u8,
    kind: PairKind,
    passphrase: Vec<u8>,
    save_contact: bool,
    same_public_ip: bool,
}

pub trait State {}

pub struct AporturePairingProtocol<S: State> {
    data: Box<AporturePairingProtocolState>,
    state: S,
}

pub trait Kind {}

pub struct Sender {}
impl State for Sender {}
impl Kind for Sender {}

impl AporturePairingProtocol<Sender> {
    #[must_use]
    pub fn new(passphrase: Vec<u8>, save_contact: bool) -> AporturePairingProtocol<Start<Sender>> {
        let state = AporturePairingProtocolState {
            protocol_version: crate::protocol::PROTOCOL_VERSION,
            kind: PairKind::Sender,
            passphrase,
            same_public_ip: false,
            save_contact,
        };

        AporturePairingProtocol {
            data: Box::new(state),
            state: Start(PhantomData),
        }
    }
}

pub struct Receiver {}
impl State for Receiver {}
impl Kind for Receiver {}

impl AporturePairingProtocol<Receiver> {
    #[must_use]
    pub fn new(
        passphrase: Vec<u8>,
        save_contact: bool,
    ) -> AporturePairingProtocol<Start<Receiver>> {
        let state = AporturePairingProtocolState {
            protocol_version: crate::protocol::PROTOCOL_VERSION,
            kind: PairKind::Receiver,
            passphrase,
            same_public_ip: false,
            save_contact,
        };

        AporturePairingProtocol {
            data: Box::new(state),
            state: Start(PhantomData),
        }
    }
}

pub struct Start<K: Kind>(PhantomData<K>);

impl<K: Kind> State for Start<K> {}

impl AporturePairingProtocol<Start<Sender>> {
    pub async fn pair(self) -> Result<PairInfo, Error> {
        let pair_info = self
            .connect()
            .await?
            .exchange_key()
            .await?
            .exchange()
            .await?;

        Ok(pair_info)
    }
}

impl AporturePairingProtocol<Start<Receiver>> {
    // TODO: notify peer if error ocurred
    pub async fn pair(self) -> Result<PairInfo, Error> {
        let mut address_collector = self.connect().await?.exchange_key().await?;

        if let Err(e) = address_collector.add_upnp().await {
            log::warn!("Could not enable upnp - {e}");
        }

        if address_collector.data.same_public_ip {
            let result = address_collector.add_local();

            if result.is_err() {
                log::warn!("Could not get a private ip from system");
            }
        }

        let pair_info = address_collector.exchange().await?;

        Ok(pair_info)
    }
}

impl<K: Kind + Send> AporturePairingProtocol<Start<K>> {
    pub async fn connect(self) -> Result<AporturePairingProtocol<KeyExchange<K>>, error::Hello> {
        let config = Config::get().await;

        log::info!(
            "Connecting to server at {}:{}",
            config.server_address,
            config.server_port
        );

        let server = TcpStream::connect((config.server_address, config.server_port)).await?;

        log::info!("Connected to server");

        let mut server = NetworkPeer::new(server);

        let id = Hasher::hash(&self.data.passphrase);

        let hello = Hello {
            version: self.data.protocol_version,
            kind: self.data.kind,
            pair_id: id,
        };

        server.write_ser(&hello).await?;

        let response = server.read_ser::<PairingResponseCode>().await?;

        let mut app = AporturePairingProtocol {
            data: self.data,
            state: KeyExchange {
                id,
                server,
                marker: PhantomData,
            },
        };

        match response {
            PairingResponseCode::Ok => Ok(app),
            PairingResponseCode::OkSamePublicIP => {
                app.data.same_public_ip = true;
                Ok(app)
            }
            PairingResponseCode::UnsupportedVersion => Err(error::Hello::ServerUnsupportedVersion),
            PairingResponseCode::NoPeer => Err(error::Hello::NoPeer),
            PairingResponseCode::MalformedMessage => Err(error::Hello::ClientError),
        }
    }
}

pub struct KeyExchange<K: Kind> {
    id: [u8; 32],
    server: NetworkPeer,
    marker: PhantomData<K>,
}

impl<K: Kind> State for KeyExchange<K> {}

impl<K: Kind + Send> AporturePairingProtocol<KeyExchange<K>> {
    pub async fn exchange_key(
        mut self,
    ) -> Result<AporturePairingProtocol<Negotiation<K>>, error::KeyExchange> {
        let password = &Password::new(&self.data.passphrase);
        let identity = &Identity::new(&self.state.id);

        let (spake, spake_msg) = Spake2::<Ed25519Group>::start_symmetric(password, identity);

        let key_exchange =
            KeyExchangePayload(spake_msg.try_into().expect("Spake message is 33 bytes"));

        log::info!("Exchanging spake key...");

        self.state.server.write_ser(&key_exchange).await?;

        let key_exchange = self.state.server.read_ser::<KeyExchangePayload>().await?;

        let key = spake.finish(&key_exchange.0)?;

        let key = Key::try_from(key).expect("Spake key is 32 bytes");

        log::info!("Key exchanged successfully");

        let mut cipher = Cipher::new(&key);

        cipher.set_associated_data(self.data.passphrase.clone());

        // NOTE: Add cipher to server to encrypt files going forward.
        let server = self.state.server.add_cipher(Arc::new(cipher));

        Ok(AporturePairingProtocol {
            data: self.data,
            state: Negotiation::new(server, key),
        })
    }
}

pub struct Negotiation<K: Kind> {
    key: Key,
    server: EncryptedNetworkPeer,
    addresses: Vec<TransferInfo>,
    marker: PhantomData<K>,
}

impl<K: Kind> State for Negotiation<K> {}

impl<K: Kind> Negotiation<K> {
    const fn new(server: EncryptedNetworkPeer, key: Key) -> Self {
        Self {
            key,
            server,
            addresses: Vec::new(),
            marker: PhantomData::<K>,
        }
    }
}

impl AporturePairingProtocol<Negotiation<Sender>> {
    pub async fn exchange(mut self) -> Result<PairInfo, error::Negotiation> {
        log::info!("Starting APP Negotiation");

        let write = NegotiationPayload {
            addresses: Vec::new(),
            save_contact: self.data.save_contact,
        };

        self.state.server.write_ser_enc(&write).await?;

        let read: NegotiationPayload = self.state.server.read_ser_enc().await?;

        log::debug!("Peer payload: {read:#?}");

        log::info!("Finished APP Negotiation");

        let (server, cipher) = self.state.server.extract_cipher();

        Ok(PairInfo {
            key: self.state.key,
            cipher,
            transfer_info: read
                .addresses
                .into_iter()
                .map(TransferInfo::Address)
                .collect(),
            server_fallback: Some(server),
            save_contact: read.save_contact,
        })
    }
}

impl AporturePairingProtocol<Negotiation<Receiver>> {
    pub async fn exchange(mut self) -> Result<PairInfo, error::Negotiation> {
        log::info!("Starting APP Negotiation");

        let addresses = self
            .state
            .addresses
            .iter()
            .map(TransferInfo::get_connection_address)
            .collect::<Vec<_>>();

        let write = NegotiationPayload {
            addresses,
            save_contact: self.data.save_contact,
        };

        self.state.server.write_ser_enc(&write).await?;

        let read: NegotiationPayload = self.state.server.read_ser_enc().await?;

        log::debug!("Peer payload: {read:#?}");

        log::info!("Finished APP Negotiation");

        let (server, cipher) = self.state.server.extract_cipher();

        Ok(PairInfo {
            key: self.state.key,
            cipher,
            transfer_info: self.state.addresses,
            server_fallback: Some(server),
            save_contact: read.save_contact,
        })
    }

    pub async fn add_upnp(&mut self) -> Result<(), upnp::Error> {
        let mut gateway = upnp::Gateway::new().await?;

        let external_address = gateway.open_port(DEFAULT_RECEIVER_PORT).await?;

        let info = TransferInfo::UPnP {
            local_port: DEFAULT_RECEIVER_PORT,
            external_address,
            gateway,
        };

        self.state.addresses.push(info);

        Ok(())
    }

    pub fn add_local(&mut self) -> Result<(), local_ip_address::Error> {
        let ip = local_ip_address::local_ip()?;

        let info = TransferInfo::Address((ip, DEFAULT_RECEIVER_PORT).into());

        self.state.addresses.push(info);

        Ok(())
    }
}

#[derive(Debug)]
pub struct PairInfo {
    key: Key,
    cipher: Arc<Cipher>,
    transfer_info: Vec<TransferInfo>,
    server_fallback: Option<NetworkPeer>,
    pub save_contact: bool,
}

impl PairInfo {
    #[must_use]
    pub fn cipher(&self) -> Arc<Cipher> {
        self.cipher.clone()
    }

    pub(crate) fn fallback(&mut self) -> Option<NetworkPeer> {
        self.server_fallback.take()
    }

    #[must_use]
    pub fn addresses(&self) -> Vec<SocketAddr> {
        self.transfer_info
            .iter()
            .map(TransferInfo::get_connection_address)
            .collect()
    }

    #[must_use]
    pub fn bind_addresses(&self) -> Vec<(SocketAddr, SocketAddr)> {
        self.transfer_info
            .iter()
            .map(|t| (t.get_bind_address(), t.get_connection_address()))
            .collect()
    }

    pub async fn finalize(self) -> Key {
        for info in self.transfer_info {
            info.finalize().await;
        }
        self.key
    }
}

#[derive(Debug)]
pub enum TransferInfo {
    Address(SocketAddr),
    UPnP {
        local_port: u16,
        external_address: SocketAddr,
        gateway: upnp::Gateway,
    },
}

impl TransferInfo {
    #[must_use]
    pub const fn get_connection_address(&self) -> SocketAddr {
        match self {
            Self::Address(a) => *a,
            Self::UPnP {
                external_address, ..
            } => *external_address,
        }
    }

    #[must_use]
    pub fn get_bind_address(&self) -> SocketAddr {
        match self {
            Self::Address(a) => match a {
                SocketAddr::V4(_) => (Ipv4Addr::UNSPECIFIED, a.port()).into(),
                SocketAddr::V6(_) => (Ipv6Addr::UNSPECIFIED, a.port()).into(),
            },
            Self::UPnP { local_port, .. } => (Ipv4Addr::UNSPECIFIED, *local_port).into(),
        }
    }

    async fn finalize(self) {
        match self {
            Self::Address(_) => (),
            Self::UPnP { mut gateway, .. } => {
                let _ = gateway.close_port().await;
            }
        }
    }
}
