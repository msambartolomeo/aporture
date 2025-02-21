#![allow(clippy::similar_names)]

use std::path::PathBuf;

use anyhow::{Result, bail};
use colored::Colorize;
use tokio::io::AsyncReadExt;

use crate::contacts::Holder;
use crate::progress;
use aporture::pairing::AporturePairingProtocol;
use aporture::transfer::AportureTransferProtocol;
use aporture::{Receiver, Sender};

pub async fn send(
    passphrase: Vec<u8>,
    save: Option<String>,
    old_contact: Option<String>,
    contacts: &mut Holder,
    path: PathBuf,
) -> Result<()> {
    let app = AporturePairingProtocol::<Sender>::new(passphrase, save.is_some());

    let mut pair_info = app.pair().await?;

    println!("{}", "Pairing Successful!!".green());

    println!(
        "Transferring file to {}...",
        "peer".bright_cyan().bold().underline()
    );

    let mut atp = AportureTransferProtocol::<Sender>::new(&mut pair_info, &path);

    let (snd, rcv) = tokio::sync::mpsc::channel(64);

    atp.add_progress_notifier(snd);
    progress::init_progress_bar(rcv);

    atp.transfer().await?;

    let save_confirmation = pair_info.save_contact;

    let key = pair_info.finalize().await;

    println!("{}", "File transferred successfully!".green());

    if let Some(name) = save {
        if save_confirmation {
            println!("Saving key for contact {}...", name.bright_blue().bold());

            let contacts = contacts.get_mut_or_init().await?;

            contacts.replace(name, old_contact, key);
        } else {
            let message = "Warning: Not saving contact because peer refused".yellow();
            println!("{message}",);
        }
    }

    Ok(())
}

pub async fn receive(
    passphrase: Vec<u8>,
    save: Option<String>,
    old_contact: Option<String>,
    contacts: &mut Holder,
    destination: Option<PathBuf>,
) -> Result<()> {
    let app = AporturePairingProtocol::<Receiver>::new(passphrase, save.is_some());

    let mut pair_info = app.pair().await?;

    println!("{}", "Pairing Successful!!".green());

    println!(
        "Receiving file from {}...",
        "peer".bright_cyan().bold().underline()
    );

    let Some(destination) = destination.or_else(aporture::fs::downloads_directory) else {
        bail!("Could not find destination directory");
    };

    let mut atp = AportureTransferProtocol::<Receiver>::new(&mut pair_info, &destination);

    let (snd, rcv) = tokio::sync::mpsc::channel(64);

    atp.add_progress_notifier(snd);
    progress::init_progress_bar(rcv);

    let path = atp.transfer().await?;

    let accepted_save_contact = pair_info.save_contact;

    let key = pair_info.finalize().await;

    println!("{}", "File received successfully!".green());
    println!("Saved in {}", path.display());

    if let Some(name) = save {
        if accepted_save_contact {
            println!("Saving key for contact {}...", name.bright_blue().bold());

            let contacts = contacts.get_mut_or_init().await?;

            contacts.replace(name, old_contact, key);
        } else {
            let message = "Warning: Not saving contact because peer refused".yellow();
            println!("{message}");
        }
    }

    Ok(())
}

pub async fn list_contacts(contacts: &Holder) -> Result<()> {
    let contacts = contacts.get_or_init().await?;

    let mut builder = tabled::builder::Builder::new();
    builder.push_record(["Name", "Added"]);
    contacts.list().for_each(|(n, t)| {
        builder.push_record([n, &t.format("%d/%m/%Y %H:%M").to_string()]);
    });
    let mut table = builder.build();
    table.with(tabled::settings::Style::markdown());
    println!("\n{table}\n");

    Ok(())
}

pub async fn delete_contact(contacts: &mut Holder, name: String) -> Result<()> {
    let contacts = contacts.get_mut_or_init().await?;

    println!("Delete contact {}? y/N", name.red());

    let confirmation = tokio::io::stdin().read_u8().await? as char;

    if confirmation.eq_ignore_ascii_case(&'y') {
        let deleted = contacts.delete(&name);
        if deleted {
            println!("Contact deleted");
            contacts.save().await?;
        } else {
            println!("Contact not found");
        }
    } else {
        println!("Canceled");
    }

    Ok(())
}

pub async fn pair_start(passphrase: Vec<u8>, name: String, contacts: &mut Holder) -> Result<()> {
    let app = AporturePairingProtocol::<Sender>::new(passphrase, true);

    let pair_info = app.pair().await?;

    println!("{}", "Pairing Successful!!".green());

    if !pair_info.save_contact {
        bail!("Peer refused to save contact".red());
    }
    let key = pair_info.finalize().await;

    println!(
        "Saving key for contact {}...",
        name.bright_blue().bold().underline()
    );

    let contacts = contacts.get_mut_or_init().await?;
    contacts.add(name, key);

    Ok(())
}

pub async fn pair_complete(passphrase: Vec<u8>, name: String, contacts: &mut Holder) -> Result<()> {
    let app = AporturePairingProtocol::<Receiver>::new(passphrase, true);

    let pair_info = app.pair().await?;

    println!("{}", "Pairing Successful!!".green());

    if !pair_info.save_contact {
        bail!("Peer refused to save contact".red());
    }
    let key = pair_info.finalize().await;

    println!(
        "Saving key for contact {}...",
        name.bright_blue().bold().underline()
    );

    let contacts = contacts.get_mut_or_init().await?;
    contacts.add(name, key);

    Ok(())
}
