use anyhow::{bail, Result};
use clap::Parser;
use colored::Colorize;

use aporture::fs::contacts::Contacts;
use args::{Cli, Commands, PairCommand};
use passphrase::Method;

mod args;
mod commands;
mod contacts;
mod passphrase;
mod progress;

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
async fn main() -> Result<()> {
    init_logger();

    let args = Cli::parse();

    let mut contacts_holder = contacts::Holder::default();

    match args.command {
        Commands::Send { path, method, save } => {
            if !path.is_file() {
                bail!("{} is not a valid file", path.display())
            }

            let passphrase_method = if let Some(passphrase) = method.passphrase {
                Method::Direct(passphrase)
            } else if let Some(ref name) = method.contact {
                let contacts = contacts_holder.get_or_init().await?;
                Method::Contact(name, contacts)
            } else {
                Method::Generate
            };
            let passphrase = passphrase::get(passphrase_method)?;

            commands::send(passphrase, save, method.contact, &mut contacts_holder, path).await?;
        }
        Commands::Receive {
            destination: path,
            method,
            save,
        } => {
            let passphrase_method = if let Some(passphrase) = method.passphrase {
                Method::Direct(passphrase)
            } else if let Some(ref name) = method.contact {
                let contacts = contacts_holder.get_or_init().await?;
                Method::Contact(name, contacts)
            } else {
                unreachable!("Guaranteed by clap");
            };
            let passphrase = passphrase::get(passphrase_method)?;

            commands::receive(passphrase, save, method.contact, &mut contacts_holder, path).await?;
        }
        Commands::Contacts => {
            if Contacts::exists() {
                commands::list_contacts(&contacts_holder).await?;
            } else {
                println!("No contacts found");
            }
        }
        Commands::Pair { command } => match command {
            PairCommand::Start { passphrase, name } => {
                let method = passphrase.map_or(Method::Generate, Method::Direct);
                let passphrase = passphrase::get(method)?;

                commands::pair_start(passphrase, name, &mut contacts_holder).await?;
            }
            PairCommand::Complete { passphrase, name } => {
                let passphrase = passphrase::get(Method::Direct(passphrase))?;

                commands::pair_complete(passphrase, name, &mut contacts_holder).await?;
            }
        },
    };

    contacts_holder.save().await?;

    println!("{}", "Success!!".green());

    Ok(())
}
