use std::marker::PhantomData;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;

use blake3::Hash;
use spake2::{Ed25519Group, Identity, Password, Spake2};
use tokio::net::TcpStream;

use crate::crypto::Cipher;
use crate::net::crypto::{EncryptedNetworkPeer, EncryptedSerdeNetwork};
use crate::net::{NetworkPeer, SerdeNetwork};
use crate::protocol::{Hello, KeyExchangePayload, PairKind, PairingResponseCode};
use crate::upnp::{self, Gateway};

pub mod error;
pub use error::Error;

const SERVER_ADDRESS: Option<&str> = option_env!("SERVER_ADDRESS");
const DEFAULT_SERVER_PORT: u16 = 8765;
const DEFAULT_RECEIVER_PORT: u16 = 8082;

pub struct AporturePairingProtocolState {
    protocol_version: u8,
    kind: PairKind,
    passphrase: Vec<u8>,
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
    pub fn new(passphrase: Vec<u8>) -> AporturePairingProtocol<Start<Sender>> {
        let state = AporturePairingProtocolState {
            protocol_version: crate::protocol::PROTOCOL_VERSION,
            kind: PairKind::Sender,
            passphrase,
            same_public_ip: false,
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
    pub fn new(passphrase: Vec<u8>) -> AporturePairingProtocol<Start<Receiver>> {
        let state = AporturePairingProtocolState {
            protocol_version: crate::protocol::PROTOCOL_VERSION,
            kind: PairKind::Receiver,
            passphrase,
            same_public_ip: false,
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
            .exchange_addr()
            .await?;

        Ok(pair_info)
    }
}

impl AporturePairingProtocol<Start<Receiver>> {
    // TODO: notify peer if error ocurred
    pub async fn pair(self) -> Result<PairInfo, Error> {
        let mut address_collector = self.connect().await?.exchange_key().await?;

        if let Err(e) = address_collector.add_upnp().await {
            log::info!("Could not enable upnp - {e}");
        }

        if address_collector.data.same_public_ip {
            let result = address_collector.add_local();

            if result.is_err() {
                log::info!("Could not get a private ip from system");
            }
        }

        let pair_info = address_collector.exchange_addr().await?;

        Ok(pair_info)
    }
}

impl<K: Kind + Send> AporturePairingProtocol<Start<K>> {
    pub async fn connect(self) -> Result<AporturePairingProtocol<KeyExchange<K>>, error::Hello> {
        let server_address = SERVER_ADDRESS.unwrap_or("127.0.0.1");

        log::info!("Connecting to server on {server_address}");

        let server = TcpStream::connect((server_address, DEFAULT_SERVER_PORT)).await?;

        let mut server = NetworkPeer::new(server);

        let id = blake3::hash(&self.data.passphrase);

        let hello = Hello {
            version: self.data.protocol_version,
            kind: self.data.kind,
            pair_id: *id.as_bytes(),
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
    id: Hash,
    server: NetworkPeer,
    marker: PhantomData<K>,
}

impl<K: Kind> State for KeyExchange<K> {}

impl<K: Kind + Send> AporturePairingProtocol<KeyExchange<K>> {
    pub async fn exchange_key(
        mut self,
    ) -> Result<AporturePairingProtocol<AddressNegotiation<K>>, error::KeyExchange> {
        let password = &Password::new(&self.data.passphrase);
        let identity = &Identity::new(self.state.id.as_bytes());

        let (spake, spake_msg) = Spake2::<Ed25519Group>::start_symmetric(password, identity);

        let key_exchange =
            KeyExchangePayload(spake_msg.try_into().expect("Spake message is 33 bytes"));

        self.state.server.write_ser(&key_exchange).await?;

        let key_exchange = self.state.server.read_ser::<KeyExchangePayload>().await?;

        let key = spake.finish(&key_exchange.0)?;
        let cipher = Arc::new(Cipher::new(key));

        // NOTE: Add cipher to server to encrypt files going forward.
        let server = self.state.server.add_cipher(cipher);

        // TODO: Exchange associated data and confirm key

        Ok(AporturePairingProtocol {
            data: self.data,
            state: AddressNegotiation::new(server),
        })
    }
}

pub struct AddressNegotiation<K: Kind> {
    server: EncryptedNetworkPeer,
    addresses: Vec<TransferInfo>,
    marker: PhantomData<K>,
}

impl<K: Kind> State for AddressNegotiation<K> {}

impl<K: Kind> AddressNegotiation<K> {
    fn new(server: EncryptedNetworkPeer) -> Self {
        Self {
            server,
            addresses: Vec::new(),
            marker: PhantomData::<K>,
        }
    }
}

impl AporturePairingProtocol<AddressNegotiation<Sender>> {
    pub async fn exchange_addr(mut self) -> Result<PairInfo, error::AddressExchange> {
        let addresses = self.state.server.read_ser_enc::<Vec<SocketAddr>>().await?;

        let (server, cipher) = self.state.server.extract_cipher();

        Ok(PairInfo {
            cipher,
            transfer_info: addresses.into_iter().map(TransferInfo::Address).collect(),
            server_fallback: Some(server),
        })
    }
}

impl AporturePairingProtocol<AddressNegotiation<Receiver>> {
    pub async fn exchange_addr(mut self) -> Result<PairInfo, error::AddressExchange> {
        let addresses = self
            .state
            .addresses
            .iter()
            .map(TransferInfo::get_connection_address)
            .collect::<Vec<_>>();

        self.state.server.write_ser_enc(&addresses).await?;

        let (server, cipher) = self.state.server.extract_cipher();

        Ok(PairInfo {
            cipher,
            transfer_info: self.state.addresses,
            server_fallback: Some(server),
        })
    }

    pub async fn add_upnp(&mut self) -> Result<(), crate::upnp::Error> {
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
    cipher: Arc<Cipher>,
    transfer_info: Vec<TransferInfo>,
    server_fallback: Option<NetworkPeer>,
}

impl PairInfo {
    #[must_use]
    pub(crate) fn cipher(&mut self) -> Arc<Cipher> {
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

    pub async fn finalize(self) {
        for info in self.transfer_info {
            info.finalize().await;
        }
    }
}

#[derive(Debug)]
pub enum TransferInfo {
    Address(SocketAddr),
    UPnP {
        local_port: u16,
        external_address: SocketAddr,
        gateway: Gateway,
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
