use std::path::PathBuf;

use adw::prelude::*;
use relm4::prelude::*;

use aporture::pairing::{AporturePairingProtocol, Receiver, Sender};

#[derive(Debug)]
pub struct AportureTransfer {
    visible: bool,
    purpose: Purpose,
}

#[derive(Debug)]
pub enum Purpose {
    Send,
    Receive,
}

#[derive(Debug)]
pub enum Output {
    Success,
    // TODO: Handle errors
    // Error,
}

#[derive(Debug)]
pub enum AportureInput {
    SendFile {
        passphrase: Vec<u8>,
        path: PathBuf,
    },
    ReceiveFile {
        passphrase: Vec<u8>,
        destination: Option<PathBuf>,
    },
}

#[relm4::component(pub)]
impl Component for AportureTransfer {
    type Init = Purpose;
    type Input = AportureInput;
    type Output = Output;
    type CommandOutput = Output;

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
                set_label: match model.purpose {
                    Purpose::Send => "Sending file...",
                    Purpose::Receive => "Receiving file...",
                }
            },

            // connect_close_request[sender] => move |_| {
            //     sender.input(DialogMsg::Hide);
            //     glib::Propagation::Stop
            // }
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {
            visible: false,
            purpose: init,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _: &Self::Root) {
        self.visible = true;

        match msg {
            AportureInput::SendFile { passphrase, path } => {
                self.purpose = Purpose::Send;

                sender.oneshot_command(async move {
                    let mut pair_info = AporturePairingProtocol::<Sender>::new(passphrase)
                        .pair()
                        .await
                        .unwrap();

                    aporture::transfer::send_file(&path, &mut pair_info)
                        .await
                        .unwrap();

                    pair_info.finalize().await;

                    Output::Success
                });
            }
            AportureInput::ReceiveFile {
                passphrase,
                destination,
            } => {
                self.purpose = Purpose::Receive;

                sender.oneshot_command(async {
                    let mut pair_info = AporturePairingProtocol::<Receiver>::new(passphrase)
                        .pair()
                        .await
                        .unwrap();

                    aporture::transfer::receive_file(destination, &mut pair_info)
                        .await
                        .unwrap();

                    pair_info.finalize().await;

                    Output::Success
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
        match message {
            Output::Success => sender
                .output(Output::Success)
                .expect("Message returned to main thread"),
        }

        self.visible = false;
    }
}
