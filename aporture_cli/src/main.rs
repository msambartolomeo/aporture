use aporture_lib::pairing::{AporturePairingProtocol, PairKind};
use args::{AportureArgs, Commands, SendMethod};

use anyhow::Result;
use clap::Parser;

mod args;

fn main() -> Result<()> {
    let args = AportureArgs::parse();

    dbg!(&args);

    match args.command {
        Commands::Send {
            file_name,
            method,
            save: _,
        } => {
            let passphrase = get_passphrase(method);
            let app = AporturePairingProtocol::new(PairKind::Sender, passphrase);

            let pair_info = app.pair();

            dbg!(pair_info);
        }
        Commands::Recieve { method, save: _ } => {
            let passphrase = method
                .passphrase
                .expect("For now providing passphrase is required")
                .into_bytes();

            let app = AporturePairingProtocol::new(PairKind::Reciever, passphrase);

            let pair_info = app.pair();

            dbg!(pair_info);
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
