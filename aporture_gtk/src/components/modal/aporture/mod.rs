use std::path::PathBuf;
use std::sync::Arc;

use adw::prelude::*;
use channel::handle_pulse;
use relm4::JoinHandle;
use relm4::prelude::*;
use relm4_icons::icon_names;
use tokio::sync::Mutex;

use aporture::fs::contacts::Contacts;

use crate::emit;

pub use error::Error;

mod channel;
mod error;
mod protocol;

#[derive(Debug)]
pub struct Peer {
    pulser: Option<JoinHandle<()>>,
    progress_bar: gtk::ProgressBar,
    progress_text: String,
    total: usize,
    current: usize,
}

#[derive(Debug, Clone, Copy)]
pub enum State {
    Initial,
    Paired,
    Compress,
    Sending(usize),
    Uncompress,
    Final,
}

#[derive(Debug)]
pub enum TransferType {
    Send(Params),
    Receive(Params),
}

impl TransferType {
    const fn icon(&self) -> &'static str {
        match self {
            Self::Send(_) => icon_names::SEND,
            Self::Receive(_) => icon_names::INBOX,
        }
    }

    fn title(&self) -> String {
        match self {
            Self::Send(params) => match params.passphrase {
                PassphraseMethod::Direct(ref passphrase) => {
                    let passphrase = String::from_utf8(passphrase.clone())
                        .expect("Should have been created via ui");

                    format!("Sending file with passphrase:\n{passphrase}")
                }
                PassphraseMethod::Contact(ref contact, ..) => {
                    format!("Sending file to contact\n{contact}")
                }
            },
            Self::Receive(params) => match params.passphrase {
                PassphraseMethod::Direct(ref passphrase) => {
                    let passphrase = String::from_utf8(passphrase.clone())
                        .expect("Should have been created via ui");

                    format!("Receiving file with passphrase:\n{passphrase}")
                }
                PassphraseMethod::Contact(ref contact, ..) => {
                    format!("Receiving file from contact\n{contact}")
                }
            },
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
    Pulse,
    Cancel,
    UpdateState(State),
    Progress(usize),
}

#[derive(Debug)]
pub enum ContactAction {
    NoOp,
    Added,
    PeerRefused,
}

#[relm4::component(pub)]
impl Component for Peer {
    type Init = TransferType;
    type Input = Msg;
    type Output = Result<ContactAction, Error>;
    type CommandOutput = Result<ContactAction, Error>;

    view! {
        dialog = adw::Window {
            set_visible: true,
            set_modal: true,
            set_title: Some("Transferring file"),

            grab_focus: (),

            set_default_width: 250,
            set_default_height: 350,

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

                    gtk::Image {
                        set_icon_name: Some(icon),
                        add_css_class: "big-icon",
                    },

                    gtk::Label {
                        set_justify: gtk::Justification::Center,

                        set_text: &title,
                    },

                    gtk::Button {
                        add_css_class: "suggested-action",

                        set_label: "Cancel",
                        connect_clicked => Msg::Cancel,
                    },

                    #[local_ref]
                    pb -> gtk::ProgressBar {
                        #[watch]
                        set_text: Some(&model.progress_text),
                        set_show_text: true,

                        set_pulse_step: 0.1,
                    },
                },
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let icon = init.icon();

        let title = init.title();

        let model = Self {
            pulser: None,
            progress_bar: gtk::ProgressBar::default(),
            progress_text: String::new(),
            total: 0,
            current: 0,
        };

        let pb = &model.progress_bar;

        let widgets = view_output!();

        match init {
            TransferType::Send(params) => {
                sender.oneshot_command(protocol::send(sender.clone(), params));
            }
            TransferType::Receive(params) => {
                sender.oneshot_command(protocol::receive(sender.clone(), params));
            }
        }

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, root: &Self::Root) {
        match msg {
            Msg::UpdateState(state) => {
                self.progress_text = match state {
                    State::Initial => {
                        self.progress_bar.set_fraction(0.0);
                        self.pulser = Some(handle_pulse(sender));
                        String::from("Waiting for peer...")
                    }
                    State::Paired => String::from("Pairing complete!"),
                    State::Compress => String::from("Compressing files before transfer..."),
                    State::Sending(total) => {
                        self.total = total;
                        self.current = 0;
                        self.pulser.take().as_ref().map(JoinHandle::abort);

                        sender.input(Msg::Progress(0));

                        String::from("0%")
                    }
                    State::Uncompress => {
                        self.progress_bar.set_fraction(0.0);
                        self.pulser = Some(handle_pulse(sender));
                        String::from("Uncompressing files...")
                    }
                    State::Final => {
                        self.pulser.take().as_ref().map(JoinHandle::abort);
                        String::from("Finished Transfer")
                    }
                };
            }

            Msg::Pulse => self.progress_bar.pulse(),

            Msg::Progress(n) => {
                self.current += n;

                #[allow(clippy::cast_precision_loss)]
                let fraction = self.current as f64 / self.total as f64;

                self.progress_bar.set_fraction(fraction);
                self.progress_text = format!("{:.2}%", fraction * 100.0);
            }

            Msg::Cancel => {
                self.pulser.take().as_ref().map(JoinHandle::abort);
                root.close();

                emit!(Err(Error::Cancel) => sender);
            }
        }
    }

    fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        sender: ComponentSender<Self>,
        root: &Self::Root,
    ) {
        emit!(message => sender);

        self.pulser.take().as_ref().map(JoinHandle::abort);
        root.close();
    }
}
