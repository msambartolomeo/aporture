use std::{ops::Not, sync::Arc};

use adw::prelude::*;
use relm4::prelude::*;
use tokio::sync::RwLock;

use crate::{
    app,
    components::dialog::peer::{self, PassphraseMethod, Peer},
};
use aporture::fs::contacts::Contacts;

#[derive(Debug)]
pub struct ReceiverPage {
    passphrase_entry: adw::EntryRow,
    contact_entry: adw::EntryRow,
    save_contact: adw::SwitchRow,
    passphrase_length: u32,
    contacts: Option<Arc<RwLock<Contacts>>>,
    aporture_dialog: Controller<Peer>,
    form_disabled: bool,
}

#[derive(Debug)]
pub enum Msg {
    ReceiveFile,
    ReceiveFileFinished,
    PassphraseChanged,
    SaveContact,
    ContactsReady(Option<Arc<RwLock<Contacts>>>),
}

#[relm4::component(pub)]
impl SimpleComponent for ReceiverPage {
    type Init = ();
    type Input = Msg;
    type Output = app::Request;

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

            #[local_ref]
            save_contact -> adw::SwitchRow {
                set_title: "Save contact",

                #[watch]
                set_sensitive: !model.form_disabled,

                connect_active_notify => Msg::SaveContact,
            },

            #[local_ref]
            contact_entry -> adw::EntryRow {
                set_title: "Contact Name",

                #[watch]
                set_sensitive: !model.form_disabled,
                #[watch]
                set_visible: model.save_contact.is_active(),
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let aporture_dialog = Peer::builder()
            .transient_for(&root)
            .launch(())
            .forward(sender.input_sender(), |_| Msg::ReceiveFileFinished); // TODO: Handle Errors

        let model = Self {
            passphrase_entry: adw::EntryRow::default(),
            contact_entry: adw::EntryRow::default(),
            save_contact: adw::SwitchRow::default(),
            passphrase_length: 0,
            contacts: None,
            aporture_dialog,
            form_disabled: false,
        };

        let passphrase_entry = &model.passphrase_entry;
        let contact_entry = &model.contact_entry;
        let save_contact = &model.save_contact;

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            Msg::ReceiveFile => {
                self.form_disabled = true;

                let passphrase = self.passphrase_entry.text();

                log::info!("Selected passphrase is {}", passphrase);

                let passphrase = PassphraseMethod::Direct(passphrase.into_bytes());

                log::info!("Starting receiver worker");

                let save = self.save_contact.is_active().not().then(|| {
                    (
                        self.contact_entry.text().to_string(),
                        self.contacts
                            .clone()
                            .expect("Must exist if contact was filled"),
                    )
                });

                self.aporture_dialog.emit(peer::Msg::ReceiveFile {
                    passphrase,
                    destination: None,
                    save,
                });
            }

            Msg::ReceiveFileFinished => {
                log::info!("Finished receiver worker");

                self.form_disabled = false;
            }

            Msg::PassphraseChanged => self.passphrase_length = self.passphrase_entry.text_length(),

            Msg::SaveContact => {
                if self.contacts.is_none() && self.save_contact.is_active() {
                    sender
                        .output_sender()
                        .send(app::Request::Contacts)
                        .expect("Controller not dropped");
                }
            }

            Msg::ContactsReady(contacts) => {
                if contacts.is_none() {
                    self.save_contact.set_active(false);
                }
                self.contacts = contacts;
            }
        }
    }
}
