use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use aporture::parser::Parser;
use aporture::protocol::HolePunchingRequest;
use net::Connection;
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::Mutex;
use tokio::task::JoinSet;

mod net;

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

const DEFAULT_PORT: u16 = 8765;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    init_logger();

    let mut handlers = JoinSet::default();

    let address = ([0, 0, 0, 0], DEFAULT_PORT).into();

    handlers.spawn(app_handler(address));
    handlers.spawn(address_handler(address));

    handlers.join_all().await.into_iter().collect()
}

async fn app_handler(address: SocketAddr) -> Result<(), std::io::Error> {
    log::info!("Binding to tcp {address}");

    let listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], DEFAULT_PORT))).await?;

    let map: Arc<Mutex<HashMap<[u8; 32], Connection>>> = Arc::default();

    log::info!("Server ready to accept connections");

    loop {
        let connection = Connection::from(listener.accept().await?);

        tokio::spawn(net::handle_connection(connection, map.clone()));
    }
}

async fn address_handler(address: SocketAddr) -> Result<(), std::io::Error> {
    log::info!("Binding to udp {address}");

    let socket = Mutex::new(Arc::new(UdpSocket::bind(address).await?));

    loop {
        let socket = socket.lock().await;
        let s = Arc::clone(&socket);
        drop(socket);
        let mut buffer = [0; 1500];

        let (len, address) = s.recv_from(&mut buffer).await?;

        log::info!("UDP connection");

        tokio::spawn(async move {
            let Ok(message) = HolePunchingRequest::deserialize_from(&buffer[..len]) else {
                log::warn!("Invalid udp connection");
                return;
            };

            match message {
                HolePunchingRequest::None => log::debug!("Keepalive holepunching from {address}"),
                HolePunchingRequest::Address => {
                    let serialized = address.serialize_to();

                    let result = s.send_to(&serialized, address).await;

                    if result.is_err() {
                        log::warn!("Unable to respond to udp connection");
                    }
                }
                HolePunchingRequest::Relay => todo!(),
            }
        });
    }
}
