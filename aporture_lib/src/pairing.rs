use std::marker::PhantomData;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

use blake3::Hash;
use spake2::{Ed25519Group, Identity, Password, Spake2};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::crypto::Cipher;
use crate::protocol::{Hello, KeyExchangePayload, PairKind, Parser, ResponseCode};
use crate::upnp::{self, Gateway};

const SERVER_ADDRESS: &str = "127.0.0.1:8080";
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
            protocol_version: 1,
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
            protocol_version: 1,
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
    pub async fn pair(self) -> Result<PairInfo, String> {
        self.connect()
            .await?
            .exchange_key()
            .await?
            .exchange_addr()
            .await
    }
}

impl AporturePairingProtocol<Start<Receiver>> {
    pub async fn pair(self) -> Result<PairInfo, String> {
        let mut address_collector = self.connect().await?.exchange_key().await?;

        address_collector.add_upnp().await;
        if address_collector.data.same_public_ip {
            address_collector.add_local().await;
        }
        address_collector.add_server().await;

        address_collector.exchange_addr().await
    }
}

impl<K: Kind> AporturePairingProtocol<Start<K>> {
    pub async fn connect(self) -> Result<AporturePairingProtocol<KeyExchange<K>>, String> {
        let mut server = TcpStream::connect(SERVER_ADDRESS)
            .await
            .expect("Connect to server");

        let id = blake3::hash(&self.data.passphrase);

        let hello = Hello {
            version: self.data.protocol_version,
            kind: self.data.kind,
            pair_id: *id.as_bytes(),
        };

        let mut response = ResponseCode::buffer();

        tcp_send_receive(&mut server, &hello, &mut response).await;

        let response = ResponseCode::deserialize_from(&response).expect("Valid response code");

        let mut app = AporturePairingProtocol {
            data: self.data,
            state: KeyExchange {
                id,
                server,
                marker: PhantomData,
            },
        };

        match response {
            ResponseCode::Ok => Ok(app),
            ResponseCode::OkSamePublicIP => {
                app.data.same_public_ip = true;
                Ok(app)
            }
            ResponseCode::UnsupportedVersion => Err("Unsupported Version".to_owned()),
            ResponseCode::NoPeer => Err("Peer has not arrived".to_owned()),
            ResponseCode::MalformedMessage => Err("Unknown error".to_owned()),
        }
    }
}

pub struct KeyExchange<K: Kind> {
    id: Hash,
    server: TcpStream,
    marker: PhantomData<K>,
}

impl<K: Kind> State for KeyExchange<K> {}

impl<K: Kind> AporturePairingProtocol<KeyExchange<K>> {
    pub async fn exchange_key(
        mut self,
    ) -> Result<AporturePairingProtocol<AddressNegotiation<K>>, String> {
        let password = &Password::new(&self.data.passphrase);
        let identity = &Identity::new(self.state.id.as_bytes());

        let (spake, spake_msg) = Spake2::<Ed25519Group>::start_symmetric(password, identity);

        let key_exchange =
            KeyExchangePayload(spake_msg.try_into().expect("Spake message is 33 bytes"));

        let mut exchange_buffer = KeyExchangePayload::buffer();

        tcp_send_receive(&mut self.state.server, &key_exchange, &mut exchange_buffer).await;

        let key_exchange =
            KeyExchangePayload::deserialize_from(&exchange_buffer).expect("Valid key exchange");

        let key = spake.finish(&key_exchange.0).expect("Key derivation works");

        let cipher = Cipher::new(key);

        // TODO: Exchange associated data and confirm key

        Ok(AporturePairingProtocol {
            data: self.data,
            state: AddressNegotiation::new(cipher, self.state.server),
        })
    }
}

pub struct AddressNegotiation<K: Kind> {
    cipher: Cipher,
    server: TcpStream,
    addresses: Vec<TransferInfo>,
    marker: PhantomData<K>,
}

impl<K: Kind> State for AddressNegotiation<K> {}

impl<K: Kind> AddressNegotiation<K> {
    fn new(cipher: Cipher, server: TcpStream) -> Self {
        Self {
            cipher,
            server,
            addresses: Vec::new(),
            marker: PhantomData::<K>,
        }
    }
}

impl AporturePairingProtocol<AddressNegotiation<Sender>> {
    pub async fn exchange_addr(mut self) -> Result<PairInfo, String> {
        let mut length = [0u8; 8];
        self.state
            .server
            .read_exact(&mut length)
            .await
            .expect("Read buffer");

        let length = usize::from_be_bytes(length);

        let mut addresses = vec![0; length];

        self.state
            .server
            .read_exact(&mut addresses)
            .await
            .expect("Read addresses");

        self.state
            .server
            .write_all(&ResponseCode::Ok.serialize_to())
            .await
            .expect("write works");

        let addresses =
            Vec::<SocketAddr>::deserialize_from(&addresses).expect("Valid socket address");

        Ok(PairInfo::Sender {
            key: self.state.cipher,
            addresses,
        })
    }
}

impl AporturePairingProtocol<AddressNegotiation<Receiver>> {
    pub async fn exchange_addr(mut self) -> Result<PairInfo, String> {
        let addresses = self
            .state
            .addresses
            .iter()
            .map(TransferInfo::get_connection_address)
            .collect::<Vec<_>>()
            .serialize_to();

        let length = addresses.len().to_be_bytes();

        self.state
            .server
            .write_all(&length)
            .await
            .expect("write length");

        self.state
            .server
            .write_all(&addresses)
            .await
            .expect("write addresses");

        let mut response = ResponseCode::buffer();

        self.state
            .server
            .read_exact(&mut response)
            .await
            .expect("Read buffer");

        let response = ResponseCode::deserialize_from(&response).expect("Valid response message");

        assert!(matches!(response, ResponseCode::Ok));

        Ok(PairInfo::Receiver {
            key: self.state.cipher,
            transfer_info: self.state.addresses,
        })
    }

    pub async fn add_upnp(&mut self) {
        let mut gateway = upnp::Gateway::new().await.expect("upnp enabled in router");

        let external_address = gateway
            .open_port(DEFAULT_RECEIVER_PORT)
            .await
            .expect("open port successfully");

        let info = TransferInfo::UPnP {
            local_port: DEFAULT_RECEIVER_PORT,
            external_address,
            gateway,
        };

        self.state.addresses.push(info);
    }

    pub async fn add_local(&mut self) {
        unimplemented!()
    }

    pub async fn add_server(&mut self) {
        unimplemented!()
    }
}

async fn tcp_send_receive<P: Parser>(stream: &mut TcpStream, input: &P, out_buf: &mut [u8]) {
    let in_buf = input.serialize_to();

    stream.write_all(&in_buf).await.expect("write hello");

    stream.read_exact(out_buf).await.expect("Read buffer");
}

#[derive(Debug)]
pub enum PairInfo {
    Sender {
        key: Cipher,
        addresses: Vec<SocketAddr>,
    },
    Receiver {
        key: Cipher,
        transfer_info: Vec<TransferInfo>,
    },
}

impl PairInfo {
    #[must_use]
    pub fn cipher(&mut self) -> &mut Cipher {
        match self {
            Self::Sender { key, .. } | Self::Receiver { key, .. } => key,
        }
    }

    #[must_use]
    pub fn addresses(&self) -> Vec<SocketAddr> {
        match self {
            Self::Sender { addresses, .. } => addresses.clone(),
            Self::Receiver { transfer_info, .. } => transfer_info
                .iter()
                .map(TransferInfo::get_connection_address)
                .collect(),
        }
    }

    pub fn bind_addresses(&self) -> Vec<(SocketAddr, SocketAddr)> {
        match self {
            Self::Sender { addresses, .. } => addresses.iter().copied().map(|a| (a, a)).collect(),
            Self::Receiver { transfer_info, .. } => transfer_info
                .iter()
                .map(|t| (t.get_bind_address(), t.get_connection_address()))
                .collect(),
        }
    }

    pub async fn finalize(self) {
        match self {
            PairInfo::Sender { .. } => (),
            PairInfo::Receiver { transfer_info, .. } => {
                for info in transfer_info {
                    info.finalize().await;
                }
            }
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
            TransferInfo::Address(_) => (),
            TransferInfo::UPnP { mut gateway, .. } => {
                let _ = gateway.close_port().await;
            }
        }
    }
}
