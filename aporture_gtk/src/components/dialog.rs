use std::path::PathBuf;

use adw::prelude::*;
use relm4::prelude::*;

use aporture::pairing::{AporturePairingProtocol, Receiver, Sender};

#[derive(Debug)]
pub struct AportureDialog {
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

#[relm4::component(pub async)]
impl SimpleAsyncComponent for AportureDialog {
    type Init = Purpose;
    type Input = AportureInput;
    type Output = Output;

    view! {
        dialog = adw::Window {
            #[watch]
            set_visible: model.visible,
            set_modal: true,

            #[wrap(Some)]
            set_child = &gtk::Label {
                set_width_request: 250,
                set_height_request: 600,
                set_halign: gtk::Align::Center,
                set_valign: gtk::Align::Center,
                #[watch]
                set_label: match model.purpose {
                    Purpose::Send => "Sending file...",
                    Purpose::Receive => "Receiving file...",
                }
            },
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        _sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let model = AportureDialog {
            visible: false,
            purpose: init,
        };
        let widgets = view_output!();
        AsyncComponentParts { model, widgets }
    }

    async fn update(&mut self, msg: Self::Input, sender: AsyncComponentSender<Self>) {
        self.visible = true;

        match msg {
            AportureInput::SendFile { passphrase, path } => {
                self.purpose = Purpose::Send;

                let mut pair_info = AporturePairingProtocol::<Sender>::new(passphrase)
                    .pair()
                    .await
                    .unwrap();

                aporture::transfer::send_file(&path, &mut pair_info).await;

                sender
                    .output(Output::Success)
                    .expect("Message returned to main thread");
            }
            AportureInput::ReceiveFile {
                passphrase,
                destination,
            } => {
                self.purpose = Purpose::Receive;

                let mut pair_info = AporturePairingProtocol::<Receiver>::new(passphrase)
                    .pair()
                    .await
                    .unwrap();

                aporture::transfer::receive_file(destination, &mut pair_info).await;

                sender
                    .output(Output::Success)
                    .expect("Message returned to main thread");
            }
        }

        self.visible = false;
    }
}
