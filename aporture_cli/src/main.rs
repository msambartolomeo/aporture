use aporture::pairing::{AporturePairingProtocol, PairKind};
use aporture::transfer;
use args::{Cli, Commands, SendMethod};

use clap::Parser;

mod args;

fn main() {
    let args = Cli::parse();

    match args.command {
        Commands::Send {
            path,
            method,
            save: _,
        } => {
            let passphrase = get_passphrase(method);
            let app = AporturePairingProtocol::new(PairKind::Sender, passphrase);

            let pair_info = app.pair();

            // dbg!(&pair_info.transfer_info);
            // dbg!(&pair_info.other_transfer_info);

            transfer::send_file(&path, &pair_info);
        }
        Commands::Recieve {
            destination,
            method,
            save: _,
        } => {
            let passphrase = method
                .passphrase
                .expect("For now providing passphrase is required")
                .into_bytes();

            let app = AporturePairingProtocol::new(PairKind::Reciever, passphrase);

            let pair_info = app.pair();

            // dbg!(&pair_info.transfer_info);
            // dbg!(&pair_info.other_transfer_info);

            transfer::recieve_file(destination, &pair_info);
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
