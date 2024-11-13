#![allow(clippy::similar_names)]

use aporture::pairing::AporturePairingProtocol;
use aporture::transfer::AportureTransferProtocol;
use aporture::{Receiver, Sender};
use relm4::ComponentSender;

use super::channel;
use super::{ContactAction, Error, Msg, Params, PassphraseMethod, Peer, State};

pub async fn send(sender: ComponentSender<Peer>, params: Params) -> Result<ContactAction, Error> {
    let passphrase = match params.passphrase {
        PassphraseMethod::Direct(p) => p,
        PassphraseMethod::Contact(name, contacts) => contacts
            .lock()
            .await
            .get(&name)
            .ok_or(Error::NoContact)?
            .to_vec(),
    };

    sender.input(Msg::UpdateState(State::Initial));

    let app = AporturePairingProtocol::<Sender>::new(passphrase, params.save.is_some());

    let mut pair_info = app.pair().await?;

    sender.input(Msg::UpdateState(State::Paired));

    let mut atp = AportureTransferProtocol::<Sender>::new(&mut pair_info, &params.path);

    let (snd, rcv) = tokio::sync::mpsc::channel(64);

    atp.add_progress_notifier(snd);

    let handle = channel::handle_progress(rcv, sender.clone());

    atp.transfer().await?;

    let _ = handle.await;

    let save_confirmation = pair_info.save_contact;

    let key = pair_info.finalize().await;

    if let Some((name, contacts)) = params.save {
        if save_confirmation {
            let mut contacts = contacts.lock().await;
            contacts.add(name, key);
            contacts.save().await.map_err(|_| Error::ContactSaving)?;
            drop(contacts);

            Ok(ContactAction::Added)
        } else {
            Ok(ContactAction::PeerRefused)
        }
    } else {
        Ok(ContactAction::NoOp)
    }
}

pub async fn receive(
    sender: ComponentSender<Peer>,
    params: Params,
) -> Result<ContactAction, Error> {
    let passphrase = match params.passphrase {
        PassphraseMethod::Direct(p) => p,
        PassphraseMethod::Contact(name, contacts) => contacts
            .lock()
            .await
            .get(&name)
            .ok_or(Error::NoContact)?
            .to_vec(),
    };

    sender.input(Msg::UpdateState(State::Initial));

    let app = AporturePairingProtocol::<Receiver>::new(passphrase, params.save.is_some());

    let mut pair_info = app.pair().await?;

    sender.input(Msg::UpdateState(State::Paired));

    let mut atp = AportureTransferProtocol::<Receiver>::new(&mut pair_info, &params.path);

    let (snd, rcv) = tokio::sync::mpsc::channel(64);

    atp.add_progress_notifier(snd);

    let handle = channel::handle_progress(rcv, sender.clone());

    atp.transfer().await?;

    let _ = handle.await;

    let save_confirmation = pair_info.save_contact;

    let key = pair_info.finalize().await;

    if let Some((name, contacts)) = params.save {
        if save_confirmation {
            let mut contacts = contacts.lock().await;
            contacts.add(name, key);
            contacts.save().await.map_err(|_| Error::ContactSaving)?;
            drop(contacts);

            Ok(ContactAction::Added)
        } else {
            Ok(ContactAction::PeerRefused)
        }
    } else {
        Ok(ContactAction::NoOp)
    }
}
