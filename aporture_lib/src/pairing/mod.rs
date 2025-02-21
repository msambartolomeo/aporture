use std::collections::HashSet;
use std::marker::PhantomData;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::Duration;

use spake2::{Ed25519Group, Identity, Password, Spake2};
use tokio::net::TcpStream;
use tokio::task::JoinHandle;

use crate::crypto::Key;
use crate::crypto::cert::{Certificate, CertificateKey};
use crate::crypto::cipher::Cipher;
use crate::crypto::hasher::Hasher;
use crate::fs::config::Config;
use crate::net::{EncryptedNetworkPeer, NetworkPeer};
use crate::parser::{EncryptedSerdeIO, Parser, SerdeIO};
use crate::protocol::{
    Hello, HolePunchingRequest, KeyExchangePayload, NegotiationPayload, PairKind,
    PairingResponseCode,
};
use crate::{Receiver, Sender, State};

mod upnp;

pub mod error;
pub use error::Error;

const ANY_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0);

pub struct AporturePairingProtocolState {
    protocol_version: u8,
    kind: PairKind,
    passphrase: Vec<u8>,
    save_contact: bool,
    same_public_ip: bool,
}

pub struct AporturePairingProtocol<S: State> {
    data: Box<AporturePairingProtocolState>,
    state: S,
}

pub trait Kind {}

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
        let mut address_collector = self.connect().await?.exchange_key().await?;

        if let Err(e) = address_collector.enable_upnp().await {
            log::warn!("Could not enable upnp - {e}");
        }

        if let Err(e) = address_collector.enable_hole_punching().await {
            log::warn!("Could not enable hole punching - {e}");
        }

        let pair_info = address_collector.exchange().await?;

        Ok(pair_info)
    }
}

impl AporturePairingProtocol<Start<Receiver>> {
    pub async fn pair(self) -> Result<PairInfo, Error> {
        let mut address_collector = self.connect().await?.exchange_key().await?;

        if let Err(e) = address_collector.enable_upnp().await {
            log::warn!("Could not enable upnp - {e}");
        }

        if let Err(e) = address_collector.enable_hole_punching().await {
            log::warn!("Could not enable hole punching - {e}");
        }

        if address_collector.data.same_public_ip {
            let result = address_collector.enable_local();

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

        let address = config.server_address();

        log::info!("Connecting to server at {address}");

        let server = TcpStream::connect(address).await?;
        drop(config);

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

        let remote_addresses = self.send_addresses().await?;
        let connecting_sockets = self.receive_addresses().await?;
        let (self_cert, peer_cert) = self.get_certs().await?;

        let (server, cipher) = self.state.server.extract_cipher();
        let binding_sockets = self
            .state
            .addresses
            .into_iter()
            .zip(remote_addresses)
            .collect();

        Ok(PairInfo {
            key: self.state.key,
            cipher,
            connecting_sockets,
            binding_sockets,
            server_fallback: Some(server),
            self_cert,
            peer_cert,
            save_contact: self.data.save_contact,
        })
    }
}

impl AporturePairingProtocol<Negotiation<Receiver>> {
    pub async fn exchange(mut self) -> Result<PairInfo, error::Negotiation> {
        log::info!("Starting APP Negotiation");

        let connecting_sockets = self.receive_addresses().await?;
        let remote_addresses = self.send_addresses().await?;
        let (self_cert, peer_cert) = self.get_certs().await?;

        let (server, cipher) = self.state.server.extract_cipher();
        let binding_sockets = self
            .state
            .addresses
            .into_iter()
            .zip(remote_addresses)
            .collect();

        Ok(PairInfo {
            key: self.state.key,
            cipher,
            connecting_sockets,
            binding_sockets,
            server_fallback: Some(server),
            self_cert,
            peer_cert,
            save_contact: self.data.save_contact,
        })
    }

    pub fn enable_local(&mut self) -> Result<(), std::io::Error> {
        let ip = local_ip_address::local_ip()
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::AddrNotAvailable))?;

        let socket = UdpSocket::bind(ANY_ADDR)?;

        let port = socket.local_addr()?.port();

        let external_address = (ip, port).into();

        let info = TransferInfo::Socket(UdpSocketAddr {
            socket,
            external_address,
            handle: None,
        });

        self.state.addresses.push(info);

        Ok(())
    }
}

impl<K: Kind + Send> AporturePairingProtocol<Negotiation<K>> {
    pub async fn enable_upnp(&mut self) -> Result<(), upnp::Error> {
        let mut gateway =
            tokio::time::timeout(Duration::from_secs(2), upnp::Gateway::new()).await??;

        let socket = UdpSocket::bind(ANY_ADDR)?;
        let local_port = socket.local_addr()?.port();

        let external_address =
            tokio::time::timeout(Duration::from_secs(2), gateway.open_port(local_port)).await??;

        let socket = UdpSocketAddr {
            socket,
            external_address,
            handle: None,
        };

        let info = TransferInfo::UPnP {
            socket,
            local_port,
            gateway,
        };

        self.state.addresses.push(info);

        Ok(())
    }

    pub async fn enable_hole_punching(&mut self) -> Result<(), crate::io::Error> {
        let socket = get_external_socket().await?;

        let info = TransferInfo::Socket(socket);

        self.state.addresses.push(info);

        Ok(())
    }

    async fn send_addresses(&mut self) -> Result<Vec<SocketAddr>, error::Negotiation> {
        let addresses = self
            .state
            .addresses
            .iter()
            .map(TransferInfo::get_connection_address)
            .collect::<Vec<_>>();

        let payload = NegotiationPayload {
            addresses,
            save_contact: self.data.save_contact,
        };

        self.state.server.write_ser_enc(&payload).await?;

        let addresses = self.state.server.read_ser_enc().await?;

        Ok(addresses)
    }

    async fn receive_addresses(
        &mut self,
    ) -> Result<Vec<(UdpSocketAddr, SocketAddr)>, error::Negotiation> {
        let payload: NegotiationPayload = self.state.server.read_ser_enc().await?;

        self.data.save_contact = self.data.save_contact && payload.save_contact;

        let mut info = Vec::new();
        for a in payload.addresses {
            let socket = get_external_socket().await?;

            info.push((socket, a));
        }

        let addresses = info
            .iter()
            .map(|s| s.0.external_address)
            .collect::<Vec<_>>();
        self.state.server.write_ser_enc(&addresses).await?;

        Ok(info)
    }

    async fn get_certs(&mut self) -> Result<(CertificateKey, Certificate), error::Negotiation> {
        let addresses = self
            .state
            .addresses
            .iter()
            .map(TransferInfo::get_connection_address)
            .map(|s| s.ip().to_string())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        log::info!("Creating certificate valid for {addresses:?}");

        let self_cert = CertificateKey::new(addresses)?;

        let der = self_cert.cert_der();

        self.state.server.write_ser_enc(&der).await?;

        let der = self.state.server.read_ser_enc::<Vec<u8>>().await?;

        let peer_cert = Certificate::from(der);

        Ok((self_cert, peer_cert))
    }
}

async fn get_external_socket() -> Result<UdpSocketAddr, crate::io::Error> {
    let config = Config::get().await;
    let server_address = config.server_address();
    drop(config);

    let socket = tokio::net::UdpSocket::bind(ANY_ADDR).await?;

    let request = HolePunchingRequest::Address.serialize_to();

    let mut address = None;

    for _ in 0..5 {
        socket.send_to(&request, server_address).await?;

        let mut buf = vec![0; 32];

        if let Ok(Ok((len, from))) =
            tokio::time::timeout(Duration::from_millis(500), socket.recv_from(&mut buf)).await
        {
            if from != server_address {
                continue;
            }

            if let Ok(a) = SocketAddr::deserialize_from(&buf[..len]) {
                if !is_private_ip(a) {
                    address = Some(a);
                }
                break;
            };
        }
    }

    let (socket, external_address, handle) = if let Some(address) = address {
        let socket = socket.into_std()?;

        let s = socket.try_clone()?;

        let request = HolePunchingRequest::None.serialize_to();

        let handle = tokio::spawn(async move {
            let _ = s.send_to(&request, server_address);
            tokio::time::sleep(Duration::from_secs(10)).await;
        });

        (socket, address, Some(handle))
    } else
    // TODO: CHANGE
    {
        let (socket, address) = stunclient::just_give_me_the_udp_socket_and_its_external_address();

        (socket, address, None)
    };

    Ok(UdpSocketAddr {
        socket,
        external_address,
        handle,
    })
}

fn is_private_ip(socket_addr: SocketAddr) -> bool {
    match socket_addr.ip() {
        IpAddr::V4(ipv4) => ipv4.is_private(),
        IpAddr::V6(_) => unreachable!("No ipv6 support"),
    }
}

#[derive(Debug)]
pub struct PairInfo {
    key: Key,
    cipher: Arc<Cipher>,
    connecting_sockets: Vec<(UdpSocketAddr, SocketAddr)>,
    binding_sockets: Vec<(TransferInfo, SocketAddr)>,
    server_fallback: Option<NetworkPeer>,
    self_cert: CertificateKey,
    peer_cert: Certificate,
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

    pub fn connecting_sockets(&self) -> impl Iterator<Item = ConnectionIdentifier<'_>> {
        self.connecting_sockets
            .iter()
            .map(ConnectionIdentifier::from)
    }

    pub fn binding_sockets(&self) -> impl Iterator<Item = ConnectionIdentifier<'_>> {
        self.binding_sockets.iter().map(ConnectionIdentifier::from)
    }

    #[must_use]
    pub fn pair_addresses(&self) -> &[(UdpSocketAddr, SocketAddr)] {
        &self.connecting_sockets
    }

    #[must_use]
    pub fn peer_certificate(&self) -> Certificate {
        self.peer_cert.clone()
    }

    #[must_use]
    pub fn self_certificate(&self) -> CertificateKey {
        self.self_cert.clone()
    }

    pub async fn finalize(self) -> Key {
        for (info, _) in self.binding_sockets {
            info.finalize().await;
        }
        self.key
    }
}

#[derive(Debug)]
pub struct UdpSocketAddr {
    socket: UdpSocket,
    external_address: SocketAddr,
    handle: Option<JoinHandle<()>>,
}

impl UdpSocketAddr {
    pub fn try_clone(&self) -> Result<UdpSocket, std::io::Error> {
        self.socket.try_clone()
    }
}

pub struct ConnectionIdentifier<'a> {
    pub local_socket: &'a UdpSocket,
    pub self_address: SocketAddr,
    pub peer_address: SocketAddr,
}

impl<'a> From<&'a (TransferInfo, SocketAddr)> for ConnectionIdentifier<'a> {
    fn from((t, a): &'a (TransferInfo, SocketAddr)) -> Self {
        ConnectionIdentifier {
            local_socket: t.get_socket(),
            self_address: t.get_connection_address(),
            peer_address: *a,
        }
    }
}

impl<'a> From<&'a (UdpSocketAddr, SocketAddr)> for ConnectionIdentifier<'a> {
    fn from((s, a): &'a (UdpSocketAddr, SocketAddr)) -> Self {
        ConnectionIdentifier {
            local_socket: &s.socket,
            self_address: s.external_address,
            peer_address: *a,
        }
    }
}

#[derive(Debug)]
pub enum TransferInfo {
    Socket(UdpSocketAddr),
    UPnP {
        socket: UdpSocketAddr,
        local_port: u16,
        gateway: upnp::Gateway,
    },
}

impl TransferInfo {
    #[must_use]
    pub const fn get_connection_address(&self) -> SocketAddr {
        match self {
            Self::Socket(UdpSocketAddr {
                external_address, ..
            })
            | Self::UPnP {
                socket: UdpSocketAddr {
                    external_address, ..
                },
                ..
            } => *external_address,
        }
    }

    #[must_use]
    pub const fn get_socket(&self) -> &UdpSocket {
        match self {
            Self::Socket(socket) | Self::UPnP { socket, .. } => &socket.socket,
        }
    }

    async fn finalize(self) {
        match self {
            Self::Socket(UdpSocketAddr { handle, .. }) => {
                handle.as_ref().map(JoinHandle::abort);
            }

            Self::UPnP { mut gateway, .. } => {
                let _ = gateway.close_port().await;
            }
        }
    }
}
