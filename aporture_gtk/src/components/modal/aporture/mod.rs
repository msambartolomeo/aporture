#![allow(clippy::similar_names)]

use std::path::PathBuf;
use std::sync::Arc;

use adw::prelude::*;
use relm4::prelude::*;
use relm4::JoinHandle;
use relm4_icons::icon_names;
use tokio::sync::Mutex;

use aporture::fs::contacts::Contacts;

use crate::emit;

pub use error::Error;
mod error;
mod protocol;

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
pub enum PassphraseMethod {
    Direct(Vec<u8>),
    Contact(String, Arc<Mutex<Contacts>>),
}

#[derive(Debug)]
pub struct Params {
    passphrase: PassphraseMethod,
    path: PathBuf,
    save: Option<(String, Arc<Mutex<Contacts>>)>,
}

impl Params {
    pub const fn new(
        passphrase: PassphraseMethod,
        path: PathBuf,
        save: Option<(String, Arc<Mutex<Contacts>>)>,
    ) -> Self {
        Self {
            passphrase,
            path,
            save,
        }
    }
}

#[derive(Debug)]
pub enum Msg {
    SendFile(Params),
    ReceiveFile(Params),
    UpdateState(State),
    Pulse,
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
            Msg::SendFile(params) => {
                self.icon = icon_names::SEND;
                self.title = match params.passphrase {
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

                sender.oneshot_command(protocol::send(sender.clone(), params));
            }

            Msg::ReceiveFile(params) => {
                self.icon = icon_names::INBOX;
                self.title = match params.passphrase {
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

                sender.oneshot_command(protocol::receive(sender.clone(), params));
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
