#![allow(clippy::similar_names)]

use std::path::PathBuf;
use std::sync::Arc;

use adw::prelude::*;
use relm4::prelude::*;
use relm4::JoinHandle;
use relm4_icons::icon_names;
use tokio::sync::Mutex;

use aporture::fs::contacts::Contacts;
use aporture::pairing::AporturePairingProtocol;
use aporture::transfer::AportureTransferProtocol;
use aporture::{Receiver, Sender};

use crate::emit;

#[derive(Debug)]
pub struct Peer {
    visible: bool,
    state: State,
    pulser: Option<JoinHandle<()>>,
    progress_bar: gtk::ProgressBar,
    icon: &'static str,
    title: String,
}

#[derive(Debug, Clone, Copy)]
pub enum State {
    Initial,
    Paired,
    Compress,
    Sending,
}

impl State {
    const fn msg(self) -> &'static str {
        match self {
            Self::Initial => "Waiting for peer...",
            Self::Paired => "Pairing complete",
            Self::Compress => {
                "The folder to send had too many files!\nPlease be patient, it will be compressed before the transfer..."
            }
            Self::Sending => "Transfering file",
        }
    }
}

#[derive(Debug)]
pub enum Msg {
    SendFile {
        passphrase: PassphraseMethod,
        path: PathBuf,
        save: Option<(String, Arc<Mutex<Contacts>>)>,
    },
    ReceiveFile {
        passphrase: PassphraseMethod,
        destination: PathBuf,
        save: Option<(String, Arc<Mutex<Contacts>>)>,
    },
    UpdateState(State),
    Pulse,
}

#[derive(Debug)]
pub enum PassphraseMethod {
    Direct(Vec<u8>),
    Contact(String, Arc<Mutex<Contacts>>),
}

#[derive(Debug)]
pub enum ContactAction {
    NoOp,
    Added,
    PeerRefused,
}

#[relm4::component(pub)]
impl Component for Peer {
    type Init = ();
    type Input = Msg;
    type Output = Result<ContactAction, Error>;
    type CommandOutput = Result<ContactAction, Error>;

    view! {
        dialog = adw::Window {
            #[watch]
            set_visible: model.visible,
            set_modal: true,
            set_title: Some("Transferring file"),

            grab_focus: (),

            set_default_width: 250,
            set_default_height: 300,

            adw::ToolbarView {
                set_top_bar_style: adw::ToolbarStyle::Flat,

                add_top_bar = &adw::HeaderBar {
                    set_show_end_title_buttons: false,
                },

                gtk::Box {
                    set_align: gtk::Align::Center,
                    set_orientation: gtk::Orientation::Vertical,
                    set_width_request: 200,
                    set_spacing: 25,

                    gtk::Image::from_icon_name(model.icon) {
                        add_css_class: "big-icon",
                    },

                    gtk::Label {
                        set_justify: gtk::Justification::Center,

                        #[watch]
                        set_text: &model.title,
                    },

                    #[local_ref]
                    pb -> gtk::ProgressBar {
                        #[watch]
                        set_text: Some(model.state.msg()),
                        set_show_text: true,

                        set_pulse_step: 0.1,
                    },
                },
            },

            connect_close_request => |_| {
                gtk::glib::Propagation::Stop
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {
            visible: false,
            state: State::Initial,
            pulser: None,
            progress_bar: gtk::ProgressBar::default(),
            icon: icon_names::SEND,
            title: String::default(),
        };

        let pb = &model.progress_bar;

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _: &Self::Root) {
        match msg {
            Msg::SendFile {
                passphrase,
                path,
                save,
            } => {
                self.icon = icon_names::SEND;
                self.title = match passphrase {
                    PassphraseMethod::Direct(ref passphrase) => {
                        let passphrase = String::from_utf8(passphrase.clone())
                            .expect("Should have been created via ui");

                        format!("Sending file with passphrase:\n{passphrase}")
                    }
                    PassphraseMethod::Contact(ref contact, ..) => {
                        format!("Sending file to contact\n{contact}")
                    }
                };
                self.visible = true;

                sender.oneshot_command(send(sender.clone(), passphrase, path, save));
            }

            Msg::ReceiveFile {
                passphrase,
                destination,
                save,
            } => {
                self.icon = icon_names::INBOX;
                self.title = match passphrase {
                    PassphraseMethod::Direct(ref passphrase) => {
                        let passphrase = String::from_utf8(passphrase.clone())
                            .expect("Should have been created via ui");

                        format!("Receiving file with passphrase:\n{passphrase}")
                    }
                    PassphraseMethod::Contact(ref contact, ..) => {
                        format!("Receiving file from contact\n{contact}")
                    }
                };
                self.visible = true;

                sender.oneshot_command(receive(sender.clone(), passphrase, destination, save));
            }

            Msg::UpdateState(state) => {
                self.state = state;

                if matches!(state, State::Initial) {
                    let handle = relm4::spawn(async move {
                        loop {
                            sender.input(Msg::Pulse);
                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        }
                    });

                    self.pulser = Some(handle);
                }
                if matches!(state, State::Sending) {
                    self.pulser.take().as_ref().map(JoinHandle::abort);
                }
            }

            Msg::Pulse => self.progress_bar.pulse(),
        }
    }

    fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        sender: ComponentSender<Self>,
        _: &Self::Root,
    ) {
        emit!(message => sender);
        self.pulser.take().as_ref().map(JoinHandle::abort);
        self.visible = false;
    }
}

async fn send(
    sender: ComponentSender<Peer>,
    passphrase: PassphraseMethod,
    path: PathBuf,
    save: Option<(String, Arc<Mutex<Contacts>>)>,
) -> Result<ContactAction, Error> {
    let passphrase = match passphrase {
        PassphraseMethod::Direct(p) => p,
        PassphraseMethod::Contact(name, contacts) => contacts
            .lock()
            .await
            .get(&name)
            .ok_or(Error::NoContact)?
            .to_vec(),
    };

    sender.input(Msg::UpdateState(State::Initial));

    let app = AporturePairingProtocol::<Sender>::new(passphrase, save.is_some());

    let mut pair_info = app.pair().await?;

    sender.input(Msg::UpdateState(State::Paired));

    log::info!("Pairing successful!");

    let atp = AportureTransferProtocol::<Sender>::new(&mut pair_info, &path);

    atp.transfer().await?;

    let save_confirmation = pair_info.save_contact;

    let key = pair_info.finalize().await;

    if let Some((name, contacts)) = save {
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

async fn receive(
    sender: ComponentSender<Peer>,
    passphrase: PassphraseMethod,
    destination: PathBuf,
    save: Option<(String, Arc<Mutex<Contacts>>)>,
) -> Result<ContactAction, Error> {
    let passphrase = match passphrase {
        PassphraseMethod::Direct(p) => p,
        PassphraseMethod::Contact(name, contacts) => contacts
            .lock()
            .await
            .get(&name)
            .ok_or(Error::NoContact)?
            .to_vec(),
    };

    sender.input(Msg::UpdateState(State::Initial));

    let app = AporturePairingProtocol::<Receiver>::new(passphrase, save.is_some());

    let mut pair_info = app.pair().await?;

    sender.input(Msg::UpdateState(State::Paired));

    let atp = AportureTransferProtocol::<Receiver>::new(&mut pair_info, &destination);

    atp.transfer().await?;

    let save_confirmation = pair_info.save_contact;

    let key = pair_info.finalize().await;

    if let Some((name, contacts)) = save {
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

pub use error::Error;

mod error {
    use aporture::pairing::error::Error as PairingError;
    use aporture::transfer::{ReceiveError, SendError};

    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum Error {
        #[error("The peer sending the file has not arrived yet")]
        NoPeer,
        #[error("Could not connect to server")]
        NoServer,
        #[error("The server is malfunctioning, please try again later")]
        InvalidServer,
        #[error("The server is malfunctioning, please try again later")]
        ServerFailure,
        #[error("Could not perform pairing with peer")]
        PairingFailure,
        #[error("The file selected is invalid")]
        FileNotFound,
        #[error("You do not have access to the file you are trying to send")]
        FilePermission,
        #[error("There was a problem in the transfered file")]
        HashMismatch,
        #[error("Could not transfer file")]
        TransferFailure,
        #[error("Contact not found")]
        NoContact,
        #[error("Could not save the contact")]
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
            log::error!("Error: {e}");

            match e {
                ReceiveError::File(_) | ReceiveError::Destination => Self::FileNotFound,
                ReceiveError::Network(_) | ReceiveError::Cipher(_) => Self::TransferFailure,
                ReceiveError::HashMismatch => Self::HashMismatch,
            }
        }
    }

    impl From<SendError> for Error {
        fn from(e: SendError) -> Self {
            log::error!("Error: {e}");

            match e {
                SendError::File(_) | SendError::Path => Self::FileNotFound,
                SendError::Subpath(_) => Self::FilePermission,
                SendError::Network(_) => Self::TransferFailure,
                SendError::HashMismatch => Self::HashMismatch,
            }
        }
    }
}
