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
    let listener = TcpListener::bind("127.0.0.1:8080").await?;

    let map: Arc<Mutex<HashMap<[u8; 64], TcpStream>>> = Arc::new(Mutex::new(HashMap::new()));

    loop {
        let (mut socket, _) = listener.accept().await?;

        let map = map.clone();

        tokio::spawn(async move {
            let mut buf = [0; 1024];

            match socket.read(&mut buf).await {
                // socket closed
                Ok(n) if n == 0 => return,
                Ok(n) => n,
                Err(e) => {
                    eprintln!("failed to read from socket; err = {:?}", e);
                    return;
                }
            };

            let hello: AporturePairingProtocol = serde_bencode::de::from_bytes(&buf).unwrap();

            let mut map = map.lock().await;
            match hello.kind {
                PairKind::Sender => {
                    map.insert(hello.pair_id, socket);
                }
                PairKind::Reciever => {
                    let sender_socket = map.get_mut(&hello.pair_id).unwrap();
                    let mut reciever_socket = socket;

                    let response = serde_bencode::to_bytes(&ResponseCode::Ok).unwrap();

                    sender_socket.write_all(&response).await.unwrap();
                    reciever_socket.write_all(&response).await.unwrap();

                    // NOTE: Delegate talking between pairs, per protocol
                    tokio::io::copy_bidirectional(sender_socket, &mut reciever_socket)
                        .await
                        .unwrap();
                }
            }
        });
    }
}
