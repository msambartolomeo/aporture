#![allow(clippy::similar_names)]

use std::path::PathBuf;
use std::sync::Arc;

use adw::prelude::*;
use relm4::prelude::*;
use tokio::sync::RwLock;

use aporture::fs::contacts::Contacts;
use aporture::pairing::AporturePairingProtocol;
use aporture::transfer::AportureTransferProtocol;
use aporture::{Receiver, Sender};

#[derive(Debug)]
pub struct Peer {
    visible: bool,
    // label: &'static str,
}

#[derive(Debug)]
pub enum Msg {
    SendFile {
        passphrase: PassphraseMethod,
        path: PathBuf,
        save: Option<(String, Arc<RwLock<Contacts>>)>,
    },
    ReceiveFile {
        passphrase: PassphraseMethod,
        destination: Option<PathBuf>,
        save: Option<(String, Arc<RwLock<Contacts>>)>,
    },
}

#[derive(Debug)]
pub enum PassphraseMethod {
    Direct(Vec<u8>),
    Contact(String, Arc<RwLock<Contacts>>),
}

#[relm4::component(pub)]
impl Component for Peer {
    type Init = ();
    type Input = Msg;
    type Output = Result<(), Error>;
    type CommandOutput = Result<(), Error>;

    view! {
        dialog = adw::Window {
            #[watch]
            set_visible: model.visible,
            set_modal: true,
            set_title: Some("Transferring file"),

            set_default_width: 250,
            set_default_height: 300,

            adw::ToolbarView {
                set_top_bar_style: adw::ToolbarStyle::Flat,

                add_top_bar = &adw::HeaderBar {
                    set_show_end_title_buttons: false,
                },

                gtk::Spinner {
                    set_size_request: (150, 150),
                    set_tooltip_text: Some("Transferring file..."),
                    set_spinning: true,
                },
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self { visible: false };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _: &Self::Root) {
        self.visible = true;
        // TODO: Show errors in screen

        match msg {
            Msg::SendFile {
                passphrase,
                path,
                save,
            } => {
                sender.oneshot_command(async move {
                    let passphrase = match passphrase {
                        PassphraseMethod::Direct(p) => p,
                        PassphraseMethod::Contact(name, contacts) => contacts
                            .read()
                            .await
                            .get(&name)
                            .ok_or(Error::NoContact)?
                            .to_vec(),
                    };

                    // self.label = "Waiting for your peer...";

                    let app = AporturePairingProtocol::<Sender>::new(passphrase, save.is_some());

                    let mut pair_info = app.pair().await?;

                    log::info!("Pairing successful!");

                    // self.label = "Pairing successful!!\nTransferring file to peer";

                    let atp = AportureTransferProtocol::<Sender>::new(&mut pair_info, &path);

                    atp.transfer().await?;

                    let save_confirmation = pair_info.save_contact;

                    let key = pair_info.finalize().await;

                    if let Some((name, contacts)) = save {
                        if save_confirmation {
                            let mut contacts = contacts.write().await;
                            contacts.add(name, key);
                            contacts.save().await.map_err(|_| Error::NoContact)?;
                            drop(contacts);
                        } else {
                            // self.label = "Warning: Not saving contact because peer refused";
                        }
                    }

                    Ok(())
                });
            }
            Msg::ReceiveFile {
                passphrase,
                destination,
                save,
            } => {
                sender.oneshot_command(async {
                    let passphrase = match passphrase {
                        PassphraseMethod::Direct(p) => p,
                        PassphraseMethod::Contact(name, contacts) => contacts
                            .read()
                            .await
                            .get(&name)
                            .ok_or(Error::ContactSaving)?
                            .to_vec(),
                    };

                    let app = AporturePairingProtocol::<Receiver>::new(passphrase, save.is_some());

                    let mut pair_info = app.pair().await?;

                    let Some(destination) = destination.or_else(aporture::fs::downloads_directory)
                    else {
                        todo!()
                    };

                    let atp =
                        AportureTransferProtocol::<Receiver>::new(&mut pair_info, &destination);

                    atp.transfer().await?;

                    let save_confirmation = pair_info.save_contact;

                    let key = pair_info.finalize().await;

                    if let Some((name, contacts)) = save {
                        if save_confirmation {
                            let mut contacts = contacts.write().await;
                            contacts.add(name, key);
                            contacts.save().await.map_err(|_| Error::ContactSaving)?;
                            drop(contacts);
                        } else {
                            // self.label = "Warning: Not saving contact because peer refused";
                        }
                    }

                    Ok(())
                });
            }
        }
    }

    fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        sender: ComponentSender<Self>,
        _: &Self::Root,
    ) {
        sender
            .output(message)
            .expect("Message returned to the main thread");

        self.visible = false;
    }
}

pub use error::Error;
mod error {
    use aporture::pairing::error::Error as PairingError;
    use aporture::transfer::{ReceiveError, SendError};

    #[derive(Debug)]
    pub enum Error {
        NoPeer,
        NoServer,
        InvalidServer,
        ServerFailure,
        PairingFailure,
        FileNotFound,
        FilePermission,
        HashMismatch,
        TransferFailure,
        NoContact,
        ContactSaving,
    }

    impl From<PairingError> for Error {
        fn from(e: PairingError) -> Self {
            log::error!("Error: {e}");

            match e {
                PairingError::Hello(e) => match e {
                    aporture::pairing::error::Hello::NoServer(_) => Self::NoServer,
                    aporture::pairing::error::Hello::NoPeer => {
                        log::warn!("Selected passphrase did not match a sender");
                        Self::NoPeer
                    }
                    aporture::pairing::error::Hello::ServerUnsupportedVersion
                    | aporture::pairing::error::Hello::ClientError => Self::InvalidServer,
                    aporture::pairing::error::Hello::ServerError(_) => Self::ServerFailure,
                },
                PairingError::KeyExchange(_) | PairingError::AddressExchange(_) => {
                    Self::PairingFailure
                }
            }
        }
    }

    impl From<ReceiveError> for Error {
        fn from(e: ReceiveError) -> Self {
            log::warn!("Error: {e}");

            match e {
                ReceiveError::File(_) | ReceiveError::Destination => Self::FileNotFound,
                ReceiveError::Network(_) | ReceiveError::Cipher(_) => Self::TransferFailure,
                ReceiveError::HashMismatch => Self::HashMismatch,
            }
        }
    }

    impl From<SendError> for Error {
        fn from(e: SendError) -> Self {
            log::warn!("Error: {e}");

            match e {
                SendError::File(_) | SendError::Path => Self::FileNotFound,
                SendError::Subpath(_) => Self::FilePermission,
                SendError::Network(_) => Self::TransferFailure,
                SendError::HashMismatch => Self::HashMismatch,
            }
        }
    }
}
