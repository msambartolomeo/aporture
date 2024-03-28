use aporture::pairing::{AporturePairingProtocol, Receiver, Sender};
use aporture::transfer;
use args::{Cli, Commands, SendMethod};

use clap::Parser;

mod args;

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

fn main() {
    init_logger();

    let args = Cli::parse();

    match args.command {
        Commands::Send {
            path,
            method,
            save: _,
        } => {
            let passphrase = get_passphrase(method);
            let app = AporturePairingProtocol::<Sender>::new(passphrase);

            let pair_info = app.pair().unwrap();

            transfer::send_file(&path, &pair_info);
        }
        Commands::Receive {
            destination,
            method,
            save: _,
        } => {
            let passphrase = method
                .passphrase
                .expect("For now providing passphrase is required")
                .into_bytes();

            let app = AporturePairingProtocol::<Receiver>::new(passphrase);

            let pair_info = app.pair().unwrap();

            transfer::receive_file(destination, &pair_info);
        }
        Commands::Contacts => todo!("Add contacts"),
        Commands::Pair { command: _ } => todo!("Add pair module"),
    };
}

fn get_passphrase(method: SendMethod) -> Vec<u8> {
    if let Some(passphrase) = method.passphrase {
        return passphrase.into_bytes();
    }

    if method.contact.is_some() {
        todo!("Add contacts")
    }

    todo!("Add password generation")
}
