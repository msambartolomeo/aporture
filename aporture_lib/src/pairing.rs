use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};

use spake2::{Ed25519Group, Identity, Password, Spake2};

use crate::protocol::{Hello, KeyExchangePayload, PairKind, Parser, ResponseCode};
use crate::upnp::{self, Gateway};

const SERVER_ADDRESS: &str = "127.0.0.1:8080";
const DEFAULT_RECIEVER_PORT: u16 = 8082;

pub struct AporturePairingProtocol {
    protocol_version: u8,
    kind: PairKind,
    passphrase: Vec<u8>,
}

impl AporturePairingProtocol {
    #[must_use]
    pub fn new(kind: PairKind, passphrase: Vec<u8>) -> Self {
        Self {
            protocol_version: 1,
            kind,
            passphrase,
        }
    }

    #[must_use]
    pub fn pair(&self) -> PairInfo {
        let mut server = TcpStream::connect(SERVER_ADDRESS).expect("Connect to server");

        let id = blake3::hash(&self.passphrase);

        let hello = Hello {
            version: self.protocol_version,
            kind: self.kind,
            pair_id: *id.as_bytes(),
        };

        let mut response = [0; ResponseCode::SERIALIZED_SIZE];

        tcp_send_recieve(&mut server, &hello, &mut response);

        let response = ResponseCode::deserialize_from(&response).expect("Valid response code");

        if matches!(response, ResponseCode::Ok) {
        } else {
            panic!("Server error");
        }

        // NOTE: Start key exchange
        let password = &Password::new(&self.passphrase);
        let identity = &Identity::new(id.as_bytes());

        let (spake, spake_msg) = Spake2::<Ed25519Group>::start_symmetric(password, identity);

        let key_exchange =
            KeyExchangePayload(spake_msg.try_into().expect("Spake message is 33 bytes"));

        let mut exchange_buffer = [0; KeyExchangePayload::SERIALIZED_SIZE];

        tcp_send_recieve(&mut server, &key_exchange, &mut exchange_buffer);

        let key_exchange =
            KeyExchangePayload::deserialize_from(&exchange_buffer).expect("Valid key exchange");

        let key = spake.finish(&key_exchange.0).expect("Key derivation works");

        // TODO: Key confirmation

        // NOTE: exchange ips and ports

        let transfer_info = match self.kind {
            PairKind::Sender => {
                let mut address = [0; SocketAddr::SERIALIZED_SIZE];

                tcp_recieve_send(&mut server, &ResponseCode::Ok, &mut address);

                let address = SocketAddr::deserialize_from(&address).expect("Valid socket address");

                TransferInfo::Address(address)
            }
            PairKind::Reciever => {
                let mut gateway = upnp::Gateway::new().expect("upnp enabled in router");

                let external_address = gateway
                    .open_port(DEFAULT_RECIEVER_PORT)
                    .expect("open port succesfully");

                let mut response = [0; ResponseCode::SERIALIZED_SIZE];

                tcp_send_recieve(&mut server, &external_address, &mut response);

                let response =
                    ResponseCode::deserialize_from(&response).expect("Valid response message");

                assert!(matches!(response, ResponseCode::Ok));

                TransferInfo::UPnP {
                    local_port: DEFAULT_RECIEVER_PORT,
                    external_address,
                    gateway,
                }
            }
        };

        server
            .shutdown(std::net::Shutdown::Both)
            .expect("correct shutdown");

        log::info!("Finished pairing: {transfer_info:#?}");

        PairInfo { key, transfer_info }
    }
}

fn tcp_send_recieve<P: Parser>(stream: &mut TcpStream, input: &P, out_buf: &mut [u8]) {
    let in_buf = input.serialize_to();

    stream.write_all(&in_buf).expect("write hello");

    let read = stream.read(out_buf).expect("Read buffer");

    assert_ne!(read, 0, "Closed from server");
}

fn tcp_recieve_send<P: Parser>(stream: &mut TcpStream, input: &P, out_buf: &mut [u8]) {
    let read = stream.read(out_buf).expect("Read buffer");

    assert_ne!(read, 0, "Closed from server");

    let in_buf = input.serialize_to();
    stream.write_all(&in_buf).expect("write hello");
}

type Key = Vec<u8>;

#[derive(Debug)]
pub struct PairInfo {
    pub key: Key,
    pub transfer_info: TransferInfo,
}

#[derive(Debug)]
pub enum TransferInfo {
    LAN {
        ip: IpAddr,
        port: u16,
    },
    Address(SocketAddr),
    UPnP {
        local_port: u16,
        external_address: SocketAddr,
        gateway: Gateway,
    },
}
