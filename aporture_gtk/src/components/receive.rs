use adw::prelude::*;
use relm4::prelude::*;

use crate::components::dialog::{AportureDialog, AportureInput, Purpose};

#[derive(Debug)]
pub struct ReceiverPage {
    passphrase: gtk::EntryBuffer,
    passphrase_empty: bool,
    aporture_dialog: Controller<AportureDialog>,
    form_disabled: bool,
}

#[derive(Debug)]
pub enum Msg {
    PassphraseChanged,
    ReceiveFile,
    ReceiveFileFinished,
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

            set_title: "Receive",
            set_description: Some("Enter the passphrase shared by the sender"),
            #[wrap(Some)]
            set_header_suffix = &gtk::Button {
                set_label: "Connect",
                #[watch]
                set_sensitive: !model.form_disabled && !model.passphrase_empty,

                connect_clicked[sender] => move |_| {
                    sender.input(Msg::ReceiveFile);
                },
            },

            gtk::Entry {
                set_tooltip_text: Some("Passphrase"),
                set_buffer: &model.passphrase,
                #[watch]
                set_sensitive: !model.form_disabled,

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
        let aporture_dialog = AportureDialog::builder()
            .transient_for(&root)
            .launch(Purpose::Send)
            .forward(sender.input_sender(), |_| Msg::ReceiveFileFinished); // TODO: Handle Errors

        let model = Self {
            passphrase: gtk::EntryBuffer::default(),
            passphrase_empty: true,
            aporture_dialog,
            form_disabled: false,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            Msg::PassphraseChanged => self.passphrase_empty = self.passphrase.length() == 0,
            Msg::ReceiveFile => {
                self.form_disabled = true;
                log::info!("Selected passphrase is {}", self.passphrase.text());

                let passphrase = self.passphrase.text().into_bytes();

                log::info!("Starting receiver worker");

                self.aporture_dialog.emit(AportureInput::ReceiveFile {
                    passphrase,
                    destination: None,
                });
            }
            Msg::ReceiveFileFinished => {
                log::info!("Finished receiver worker");

                self.form_disabled = false;
            }
        }
    }
}
