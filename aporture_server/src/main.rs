use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::{serde_as, Bytes};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

#[serde_as]
#[derive(Deserialize, Serialize)]
struct AporturePairingProtocol {
    /// Protocol version
    version: u8,

    /// Pair Kind
    kind: PairKind,

    #[serde_as(as = "Bytes")]
    pair_id: [u8; 64],
}

#[derive(Deserialize_repr, Serialize_repr)]
#[repr(u8)]
enum PairKind {
    Sender = 0,
    Reciever = 1,
}

#[derive(Deserialize_repr, Serialize_repr)]
#[repr(u8)]
enum ResponseCode {
    Ok = 0,
    UnsupportedVersion = 1,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("0.0.0.0:8080").await?;

    let map: Arc<Mutex<HashMap<[u8; 64], TcpStream>>> = Arc::new(Mutex::new(HashMap::new()));

    loop {
        let (mut socket, _) = listener.accept().await?;

        let map = map.clone();

        tokio::spawn(async move {
            let mut buf = [0; 1024];

            match socket.read(&mut buf).await {
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
                    map.insert(hello.pair_id, socket);
                }
                PairKind::Reciever => {
                    let mut sender_socket =
                        map.remove(&hello.pair_id).expect("Sender already arrived");
                    // NOTE: Drop map to allow other connections
                    drop(map);
                    let mut reciever_socket = socket;

                    let response = serde_bencode::to_bytes(&ResponseCode::Ok)
                        .expect("Response code can be turn into bencode");

                    sender_socket
                        .write_all(&response)
                        .await
                        .expect("No network errors sending response");
                    reciever_socket
                        .write_all(&response)
                        .await
                        .expect("No network errors sending response");

                    // NOTE: Delegate talking between pairs, per protocol
                    tokio::io::copy_bidirectional(&mut sender_socket, &mut reciever_socket)
                        .await
                        .expect("No network errors in pairing");
                }
            }
        });
    }
}
