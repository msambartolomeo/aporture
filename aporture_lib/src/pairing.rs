use std::io::{Read, Write};
use std::net::{IpAddr, TcpStream};

use blake3::Hash;
use local_ip_address::local_ip;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::{serde_as, Bytes, DisplayFromStr};
use spake2::{Ed25519Group, Identity, Password, Spake2};

const SERVER_ADDRESS: &str = "127.0.0.1:8080";
const DEFAULT_SENDER_PORT: u16 = 8081;
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
        let mut client_buffer = [0u8; 1024];

        let mut server = TcpStream::connect(SERVER_ADDRESS).expect("Connect to server");

        let id = blake3::hash(&self.passphrase);

        let hello = ConnectionPayload {
            version: self.protocol_version,
            kind: self.kind,
            pair_id: id,
        };

        tcp_send_recieve(&mut server, &hello, &mut client_buffer);

        let response: ResponseCode =
            serde_bencode::from_bytes(&client_buffer).expect("server responds correctly");

        if matches!(response, ResponseCode::Ok) {
        } else {
            panic!("Server error");
        }

        // NOTE: Start key exchange
        let password = &Password::new(&self.passphrase);
        let identity = &Identity::new(id.as_bytes());

        let (spake, spake_msg) = Spake2::<Ed25519Group>::start_symmetric(password, identity);

        let key_exchange =
            KeyExchangePayload(spake_msg.try_into().expect("spake message is 33 chars"));

        tcp_send_recieve(&mut server, &key_exchange, &mut client_buffer);

        let key_exchange: KeyExchangePayload =
            serde_bencode::from_bytes(&client_buffer).expect("server responds correctly");

        let key = spake.finish(&key_exchange.0).expect("Key derivation works");

        // TODO: Key confirmation

        // NOTE: exchange ips and ports

        let port = match self.kind {
            PairKind::Sender => DEFAULT_SENDER_PORT,
            PairKind::Reciever => DEFAULT_RECIEVER_PORT,
        };

        let self_transfer_info = TransferType::LAN {
            ip: local_ip().expect("get local ip"),
            port,
        };

        let other_transfer_info: TransferType = match self.kind {
            PairKind::Sender => {
                tcp_send_recieve(
                    &mut server,
                    &TransferNegotaitionPayload(vec![self_transfer_info]),
                    &mut client_buffer,
                );

                serde_bencode::from_bytes(&client_buffer).expect("server responds correctly")
            }
            PairKind::Reciever => {
                tcp_recieve_send(&mut server, &self_transfer_info, &mut client_buffer);

                let v: TransferNegotaitionPayload =
                    serde_bencode::from_bytes(&client_buffer).expect("server responds correctly");

                v.0[0]
            }
        };

        server
            .shutdown(std::net::Shutdown::Both)
            .expect("correct shutdown");

        PairInfo {
            key,
            self_transfer_info,
            other_transfer_info,
        }
    }
}

fn tcp_send_recieve<S: Serialize>(stream: &mut TcpStream, input: &S, out_buf: &mut [u8]) {
    let in_buf = serde_bencode::to_bytes(input).expect("Correct serde parse");

    stream.write_all(&in_buf).expect("write hello");

    let read = stream.read(out_buf).expect("Read buffer");

    assert_eq!(read, 0, "Closed from server");
}

fn tcp_recieve_send<S: Serialize>(stream: &mut TcpStream, input: &S, out_buf: &mut [u8]) {
    let read = stream.read(out_buf).expect("Read buffer");

    assert_eq!(read, 0, "Closed from server");

    let in_buf = serde_bencode::to_bytes(input).expect("Correct serde parse");
    stream.write_all(&in_buf).expect("write hello");
}

#[serde_as]
#[derive(Debug, Deserialize, Serialize)]
struct ConnectionPayload {
    /// Protocol version
    version: u8,

    /// Pair Kind
    kind: PairKind,

    /// The hash of the passphrase
    #[serde_as(as = "DisplayFromStr")]
    pair_id: Hash,
}

#[derive(Debug, Clone, Copy, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum PairKind {
    Sender = 0,
    Reciever = 1,
}

#[derive(Debug, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum ResponseCode {
    Ok = 0,
    UnsupportedVersion = 1,
}

#[serde_as]
#[derive(Debug, Deserialize, Serialize)]
struct KeyExchangePayload(#[serde_as(as = "Bytes")] [u8; 33]);

// TODO: Do key confirmation
#[serde_as]
#[derive(Debug, Deserialize, Serialize)]
struct KeyConfirmationPayload(#[serde_as(as = "Bytes")] [u8; 33]);

#[derive(Debug, Deserialize, Serialize)]
struct TransferNegotaitionPayload(Vec<TransferType>);

type Key = Vec<u8>;
#[derive(Debug)]
pub struct PairInfo {
    pub key: Key,
    pub self_transfer_info: TransferType,
    pub other_transfer_info: TransferType,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TransferType {
    LAN { ip: IpAddr, port: u16 },
}
