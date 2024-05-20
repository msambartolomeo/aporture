use adw::prelude::*;
use relm4::prelude::*;

use crate::components::dialog::{AportureInput, AportureTransfer, Purpose};

#[derive(Debug)]
pub struct ReceiverPage {
    passphrase_entry: adw::EntryRow,
    passphrase_length: u32,
    aporture_dialog: Controller<AportureTransfer>,
    form_disabled: bool,
}

#[derive(Debug)]
pub enum Msg {
    ReceiveFile,
    ReceiveFileFinished,
    PassphraseChanged,
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

            set_width_request: 250,

            set_title: "Receive",
            set_description: Some("Enter the passphrase shared by the sender"),
            #[wrap(Some)]
            set_header_suffix = &gtk::Button {
                set_label: "Connect",
                #[watch]
                set_sensitive: !model.form_disabled && model.passphrase_length != 0,

                connect_clicked[sender] => move |_| {
                    sender.input(Msg::ReceiveFile);
                },
            },

            #[local_ref]
            passphrase_entry -> adw::EntryRow {
                set_title: "Passphrase",
                #[watch]
                set_sensitive: !model.form_disabled,

                connect_changed[sender] => move |_| {
                    sender.input(Msg::PassphraseChanged);
                }
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let aporture_dialog = AportureTransfer::builder()
            .transient_for(&root)
            .launch(Purpose::Send)
            .forward(sender.input_sender(), |_| Msg::ReceiveFileFinished); // TODO: Handle Errors

        let model = Self {
            passphrase_entry: adw::EntryRow::default(),
            passphrase_length: 0,
            aporture_dialog,
            form_disabled: false,
        };

        let passphrase_entry = &model.passphrase_entry;

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            Msg::ReceiveFile => {
                self.form_disabled = true;

                let passphrase = self.passphrase_entry.text();

                log::info!("Selected passphrase is {}", passphrase);

                let passphrase = passphrase.into_bytes();

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
            Msg::PassphraseChanged => self.passphrase_length = self.passphrase_entry.text_length(),
        }
    }
}
