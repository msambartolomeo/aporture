#![allow(clippy::similar_names)]

use std::path::PathBuf;
use std::sync::Arc;

use adw::prelude::*;
use gtk::glib::clone;
use relm4::prelude::*;
use relm4::JoinHandle;
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
}

#[derive(Debug, Clone, Copy)]
pub enum State {
    Initial,
    Paired,
    Compress,
    Sending,
}

impl State {
    fn msg(&self) -> &'static str {
        match self {
            State::Initial => "Waiting for peer...",
            State::Paired => "Pairing complete",
            State::Compress => {
                "The folder to send had too many files!\nPlease be patient, it will be compressed before the transfer..."
            }
            State::Sending => "Transfering file",
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
pub enum ContactResult {
    NoOp,
    Added,
    PeerRefused,
}

#[relm4::component(pub)]
impl Component for Peer {
    type Init = ();
    type Input = Msg;
    type Output = Result<ContactResult, Error>;
    type CommandOutput = Result<ContactResult, Error>;

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
        };

        let pb = &model.progress_bar;

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
                sender.oneshot_command(clone!(
                    #[strong]
                    sender,
                    async move {
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

                        let app =
                            AporturePairingProtocol::<Sender>::new(passphrase, save.is_some());

                        let mut pair_info = app.pair().await?;

                        sender.input(Msg::UpdateState(State::Paired));

                        log::info!("Pairing successful!");

                        let atp = AportureTransferProtocol::<Sender>::new(&mut pair_info, &path);

                        atp.transfer().await?;

                        let save_confirmation = pair_info.save_contact;

                        let key = pair_info.finalize().await;

                        let result = if let Some((name, contacts)) = save {
                            if save_confirmation {
                                let mut contacts = contacts.lock().await;
                                contacts.add(name, key);
                                contacts.save().await.map_err(|_| Error::ContactSaving)?;
                                drop(contacts);

                                ContactResult::Added
                            } else {
                                ContactResult::PeerRefused
                            }
                        } else {
                            ContactResult::NoOp
                        };

                        Ok(result)
                    }
                ));
            }

            Msg::ReceiveFile {
                passphrase,
                destination,
                save,
            } => {
                sender.oneshot_command(clone!(
                    #[strong]
                    sender,
                    async move {
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

                        let app =
                            AporturePairingProtocol::<Receiver>::new(passphrase, save.is_some());

                        // TODO: CHANGE
                        let mut pair_info = app.pair().await?;

                        sender.input(Msg::UpdateState(State::Paired));

                        let atp =
                            AportureTransferProtocol::<Receiver>::new(&mut pair_info, &destination);

                        atp.transfer().await?;

                        let save_confirmation = pair_info.save_contact;

                        let key = pair_info.finalize().await;

                        let result = if let Some((name, contacts)) = save {
                            if save_confirmation {
                                let mut contacts = contacts.lock().await;
                                contacts.add(name, key);
                                contacts.save().await.map_err(|_| Error::ContactSaving)?;
                                drop(contacts);

                                ContactResult::Added
                            } else {
                                ContactResult::PeerRefused
                            }
                        } else {
                            ContactResult::NoOp
                        };

                        Ok(result)
                    }
                ));
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
