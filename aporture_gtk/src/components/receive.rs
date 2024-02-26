use adw::prelude::*;
use relm4::{prelude::*, WorkerController};

use crate::workers::{AportureInput, AportureWorker};

#[derive(Debug)]
pub struct ReceiverPage {
    passphrase: gtk::EntryBuffer,
    passphrase_empty: bool,
    aporture_worker: WorkerController<AportureWorker>,
}

#[derive(Debug)]
pub enum Msg {
    PassphraseChanged,
    RecieveFile,
    RecievingSuccess,
}

#[relm4::component(pub)]
impl SimpleComponent for ReceiverPage {
    type Init = ();
    type Input = Msg;
    type Output = ();

    view! {
        adw::PreferencesGroup {
            set_margin_horizontal: 20,
            set_margin_vertical: 50,

            set_title: "Recieve",
            #[wrap(Some)]
            set_header_suffix = &gtk::Button {
                set_label: "Connect",
                #[watch]
                set_sensitive: !model.passphrase_empty,

                connect_clicked[sender] => move |_| {
                    sender.input(Msg::RecieveFile);
                },
            },
            set_description: Some("Enter the passphrase shared by the sender"),

            gtk::Entry {
                set_tooltip_text: Some("Passphrase"),
                set_buffer: &model.passphrase,

                connect_changed[sender] => move |_| {
                    sender.input(Msg::PassphraseChanged);
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let aporture_worker = AportureWorker::builder()
            .detach_worker(())
            .forward(sender.input_sender(), |_| Msg::RecievingSuccess); // TODO: Handle Errors

        let model = Self {
            passphrase: gtk::EntryBuffer::default(),
            passphrase_empty: true,
            aporture_worker,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            Msg::PassphraseChanged => self.passphrase_empty = self.passphrase.length() == 0,
            Msg::RecieveFile => {
                log::info!("Selected passphrase is {}", self.passphrase);

                let passphrase = self.passphrase.text().into_bytes();

                self.aporture_worker
                    .sender()
                    .emit(AportureInput::RecieveFile {
                        passphrase,
                        destination: None,
                    });
            }
            Msg::RecievingSuccess => (),
        }
    }
}
