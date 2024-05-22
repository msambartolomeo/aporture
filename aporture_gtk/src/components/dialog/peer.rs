use std::path::PathBuf;
use std::sync::Arc;

use adw::prelude::*;
use relm4::prelude::*;
use tokio::sync::RwLock;

use aporture::fs::contacts::Contacts;
use aporture::pairing::{AporturePairingProtocol, Receiver, Sender};

#[derive(Debug)]
pub struct Peer {
    visible: bool,
    label: &'static str,
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
        dialog = gtk::Window {
            #[watch]
            set_visible: model.visible,
            set_modal: true,

            #[wrap(Some)]
            set_child = &gtk::Label {
                set_width_request: 250,
                set_height_request: 400,
                set_halign: gtk::Align::Center,
                set_valign: gtk::Align::Center,
                #[watch]
                set_label: model.label,
            },

            // connect_close_request[sender] => move |_| {
            //     sender.input(DialogMsg::Hide);
            //     glib::Propagation::Stop
            // }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {
            visible: false,
            label: "",
        };

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

                    aporture::transfer::send_file(&path, &mut pair_info).await?;

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

                    aporture::transfer::receive_file(destination, &mut pair_info).await?;

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
                ReceiveError::File(_) | ReceiveError::Directory => Self::FileNotFound,
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
                SendError::Network(_) => Self::TransferFailure,
                SendError::HashMismatch => Self::HashMismatch,
            }
        }
    }
}
