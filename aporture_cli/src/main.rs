use anyhow::bail;
use clap::Parser;

use aporture::fs::contacts::Contacts;
use aporture::pairing::{AporturePairingProtocol, Receiver, Sender};
use args::{Cli, Commands, PairCommand};
use passphrase::Method;

mod args;
mod contacts;
mod passphrase;

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logger();

    let args = Cli::parse();

    match args.command {
        Commands::Send { path, method, save } => {
            let mut contacts_holder = contacts::Holder::default();

            let passphrase_method = if let Some(passphrase) = method.passphrase {
                Method::Direct(passphrase)
            } else if let Some(ref name) = method.contact {
                let contacts = contacts_holder.get_or_init().await?;
                Method::Contact(name, contacts)
            } else {
                Method::Generate
            };
            let passphrase = passphrase::get(passphrase_method)?;

            let app = AporturePairingProtocol::<Sender>::new(passphrase, save.is_some());

            let mut pair_info = app.pair().await?;

            aporture::transfer::send_file(&path, &mut pair_info).await?;

            let accepted_save_contact = pair_info.save_contact;

            let key = pair_info.finalize().await;

            if let Some(name) = save {
                if accepted_save_contact {
                    let contacts = contacts_holder.get_or_init().await?;

                    if let Some(name) = method.contact {
                        contacts.delete(&name);
                    }

                    contacts.add(name, key);
                } else {
                    println!("Warning: Not saving contact because peer refused");
                }
            }

            contacts_holder.save().await?;
        }
        Commands::Receive {
            destination,
            method,
            save,
        } => {
            let mut contacts_holder = contacts::Holder::default();

            let passphrase_method = if let Some(passphrase) = method.passphrase {
                Method::Direct(passphrase)
            } else if let Some(ref name) = method.contact {
                let contacts = contacts_holder.get_or_init().await?;
                Method::Contact(name, contacts)
            } else {
                unreachable!("Guaranteed by clap");
            };

            let passphrase = passphrase::get(passphrase_method)?;

            let app = AporturePairingProtocol::<Receiver>::new(passphrase, save.is_some());

            let mut pair_info = app.pair().await?;

            aporture::transfer::receive_file(destination, &mut pair_info).await?;

            let accepted_save_contact = pair_info.save_contact;

            let key = pair_info.finalize().await;

            if let Some(name) = save {
                if accepted_save_contact {
                    let contacts = contacts_holder.get_or_init().await?;

                    if let Some(name) = method.contact {
                        contacts.delete(&name);
                    }

                    contacts.add(name, key);
                } else {
                    println!("Warning: Not saving contact because peer refused");
                }
            }

            contacts_holder.save().await?;
        }
        Commands::Contacts => {
            if Contacts::exists() {
                let mut holder = contacts::Holder::default();
                let contacts = holder.get_or_init().await?;

                println!("Registered contacts:");
                for (name, timestamp) in contacts.list() {
                    println!("Name: {name} \t\t Added: {timestamp}");
                }
            } else {
                println!("No contacts found");
            }
        }
        Commands::Pair { command } => match command {
            PairCommand::Start { passphrase, name } => {
                let method = passphrase.map_or(Method::Generate, Method::Direct);

                let passphrase = passphrase::get(method)?;

                let app = AporturePairingProtocol::<Sender>::new(passphrase, true);

                let pair_info = app.pair().await?;

                if !pair_info.save_contact {
                    bail!("Peer refused to save contact");
                }

                let mut contacts_holder = contacts::Holder::default();
                let contacts = contacts_holder.get_or_init().await?;

                let key = pair_info.finalize().await;

                contacts.add(name, key);
            }
            PairCommand::Complete { passphrase, name } => {
                let passphrase = passphrase::get(Method::Direct(passphrase))?;

                let app = AporturePairingProtocol::<Receiver>::new(passphrase, true);

                let pair_info = app.pair().await?;

                if !pair_info.save_contact {
                    bail!("Peer refused to save contact");
                }

                let mut contacts_holder = contacts::Holder::default();

                let contacts = contacts_holder.get_or_init().await?;

                let key = pair_info.finalize().await;

                contacts.add(name, key);
            }
        },
    };

    Ok(())
}
