use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::{serde_as, Bytes};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

#[serde_as]
#[derive(Debug, Deserialize, Serialize)]
struct AporturePairingProtocol {
    /// Protocol version
    version: u8,

    /// Pair Kind
    kind: PairKind,

    #[serde_as(as = "Bytes")]
    pair_id: [u8; 64],
}

#[derive(Debug, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
enum PairKind {
    Sender = 0,
    Reciever = 1,
}

#[derive(Deserialize_repr, Serialize_repr)]
#[repr(u8)]
enum ResponseCode {
    // NOTE: Okay types
    Ok = 0,
    OkSamePublicIP = 3,

    // NOTE: Error types
    UnsupportedVersion = 1,
}

fn init_logger() {
    use std::io::Write;

    env_logger::Builder::from_default_env()
        .format(|buf, record| {
            let color = buf.default_level_style(record.level());

            writeln!(
                buf,
                "{}:{} {} {color}{}{color:#} - {}",
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                buf.timestamp(),
                record.level(),
                record.args()
            )
        })
        .init();
}

struct Connection {
    socket: TcpStream,
    address: SocketAddr,
}

impl From<(TcpStream, SocketAddr)> for Connection {
    fn from((socket, address): (TcpStream, SocketAddr)) -> Self {
        Self { socket, address }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logger();

    let listener = TcpListener::bind("0.0.0.0:8080").await?;

    let map: Arc<Mutex<HashMap<[u8; 64], Connection>>> = Arc::default();

    loop {
        let mut connection = Connection::from(listener.accept().await?);

        let map = map.clone();

        tokio::spawn(async move {
            let mut buf = [0; 1024];

            match connection.socket.read(&mut buf).await {
                // socket closed
                Ok(0) => return,
                Ok(n) => n,
                Err(e) => {
                    log::error!("failed to read from socket; err = {e:?}");
                    return;
                }
            };

            let hello: AporturePairingProtocol =
                serde_bencode::de::from_bytes(&buf).expect("Response is valid APP");

            let mut map = map.lock().await;
            match hello.kind {
                PairKind::Sender => {
                    log::info!("recieved hello from sender");
                    map.insert(hello.pair_id, connection);
                }
                PairKind::Reciever => {
                    let mut receiver = connection;

                    log::info!("recieved hello from reciever");
                    let mut sender = map.remove(&hello.pair_id).expect("Sender already arrived");

                    // NOTE: Drop map to allow other connections
                    drop(map);

                    let response = if receiver.address.ip() == sender.address.ip() {
                        ResponseCode::OkSamePublicIP
                    } else {
                        ResponseCode::Ok
                    };

                    let response = serde_bencode::to_bytes(&response)
                        .expect("Response code can be turn into bencode");

                    sender
                        .socket
                        .write_all(&response)
                        .await
                        .expect("No network errors sending response");

                    receiver
                        .socket
                        .write_all(&response)
                        .await
                        .expect("No network errors sending response");

                    log::info!("Starting bidirectional APP");

                    // NOTE: Delegate talking between pairs, per protocol
                    tokio::io::copy_bidirectional(&mut sender.socket, &mut receiver.socket)
                        .await
                        .expect("No network errors in pairing");

                    log::info!("Finished pairing");
                }
            }
        });
    }
}
