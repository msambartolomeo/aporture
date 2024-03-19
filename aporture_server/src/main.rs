use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use net::Connection;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

mod net;
mod protocol;

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

const DEFAULT_PORT: u16 = 8080;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    init_logger();

    let listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], DEFAULT_PORT))).await?;

    let map: Arc<Mutex<HashMap<[u8; 64], Connection>>> = Arc::default();

    loop {
        let connection = Connection::from(listener.accept().await?);

        tokio::spawn(net::handle_connection(connection, map.clone()));
    }
}
