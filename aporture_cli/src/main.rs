use aporture::pairing::{AporturePairingProtocol, PairKind};
use aporture::transfer;
use args::{AportureArgs, Commands, SendMethod};

use anyhow::Result;
use clap::Parser;

mod args;

fn main() -> Result<()> {
    let args = AportureArgs::parse();

    match args.command {
        Commands::Send {
            path,
            method,
            save: _,
        } => {
            let passphrase = get_passphrase(method);
            let app = AporturePairingProtocol::new(PairKind::Sender, passphrase);

            let pair_info = app.pair();

            dbg!(&pair_info.self_transfer_info);
            dbg!(&pair_info.other_transfer_info);

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

            dbg!(&pair_info.self_transfer_info);
            dbg!(&pair_info.other_transfer_info);

            transfer::recieve_file(destination, &pair_info);
        }
        Commands::Contacts => todo!("Add contacts"),
        Commands::Pair { command: _ } => todo!("Add pair module"),
    };

    Ok(())
}

fn get_passphrase(method: SendMethod) -> Vec<u8> {
    if let Some(passphrase) = method.passphrase {
        return passphrase.into_bytes();
    }

    if let Some(_) = method.contact {
        todo!("Add contacts")
    }

    todo!("Add password generation")
}
