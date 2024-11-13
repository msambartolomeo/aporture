use std::path::PathBuf;
use std::sync::Arc;

use adw::prelude::*;
use channel::handle_pulse;
use relm4::prelude::*;
use relm4::JoinHandle;
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
    visible: bool,
    pulser: Option<JoinHandle<()>>,
    progress_bar: gtk::ProgressBar,
    progress_text: String,
    total: usize,
    current: usize,
    icon: &'static str,
    title: String,
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
                        set_text: Some(&model.progress_text),
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
            pulser: None,
            progress_bar: gtk::ProgressBar::default(),
            icon: icon_names::SEND,
            title: String::default(),
            progress_text: String::new(),
            total: 0,
            current: 0,
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
                self.progress_text = match state {
                    State::Initial => {
                        self.pulser = Some(handle_pulse(sender));
                        String::from("Waiting for peer...")
                    }
                    State::Paired => String::from("Pairing complete!"),
                    State::Compress => String::from("Compressing files before transfer..."),
                    State::Sending(total) => {
                        self.total = total;
                        self.current = 0;
                        self.pulser.take().as_ref().map(JoinHandle::abort);

                        String::from("0%")
                    }
                    State::Uncompress => {
                        self.pulser = Some(handle_pulse(sender));
                        String::from("Uncompressing files...")
                    }
                    State::Final => {
                        self.pulser = Some(handle_pulse(sender));
                        String::from("Finished Transfer")
                    }
                };
            }

            Msg::Pulse => self.progress_bar.pulse(),

            Msg::Progress(n) => {
                self.current += n;

                #[allow(clippy::cast_precision_loss)]
                self.progress_bar
                    .set_fraction(self.current as f64 / self.total as f64);
            }
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
