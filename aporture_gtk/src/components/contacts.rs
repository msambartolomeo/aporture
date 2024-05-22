use std::path::PathBuf;
use std::sync::Arc;

use adw::prelude::*;
use relm4::prelude::*;
use relm4_components::open_dialog::{
    OpenDialog, OpenDialogMsg, OpenDialogResponse, OpenDialogSettings,
};
use relm4_icons::icon_names;
use tokio::sync::RwLock;

use crate::app;
use crate::components::dialog::peer::{self, PassphraseMethod, Peer};
use aporture::fs::contacts::Contacts;
use aporture::passphrase;

const PASSPHRASE_WORD_COUNT: usize = 3;

#[derive(Debug)]
pub struct SenderPage {
    contacts: Option<Arc<RwLock<Contacts>>>,
    aporture_dialog: Controller<Peer>,
    form_disabled: bool,
}

#[derive(Debug)]
pub enum Msg {
    ContactsReady(Option<Arc<RwLock<Contacts>>>),
    SendFile,
    SendFileFinished,
}

#[relm4::component(pub)]
impl SimpleComponent for SenderPage {
    type Init = ();
    type Input = Msg;
    type Output = app::Request;

    view! {
        adw::PreferencesGroup {
            set_margin_horizontal: 20,
            set_margin_vertical: 50,

            set_title: "Send",
            set_description: Some("Enter a passphrase or generate a random one"),
            #[wrap(Some)]
            set_header_suffix = &gtk::Button {
                add_css_class: "suggested-action",

                set_label: "Connect",
                #[watch]
                set_sensitive: !model.form_disabled && model.passphrase_length != 0 && model.file_path.is_some(),
                connect_clicked => Msg::SendFile,
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
            .forward(sender.input_sender(), |_| Msg::SendFileFinished); // TODO: Handle Errors

        let model = Self {
            contacts: None,
            aporture_dialog,
            form_disabled: false,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            Msg::ContactsReady(contacts) => {
                self.contacts = contacts;
            }

            Msg::SendFile => {
                self.form_disabled = true;

                let passphrase = self.passphrase_entry.text();

                log::info!("Selected passphrase is {}", passphrase);

                let passphrase = PassphraseMethod::Direct(passphrase.into_bytes());

                let save = self.save_contact.is_active().then(|| {
                    (
                        self.contact_entry.text().to_string(),
                        self.contacts
                            .clone()
                            .expect("Must exist if contact was filled"),
                    )
                });

                log::info!("Starting sender worker");

                self.aporture_dialog.emit(peer::Msg::SendFile {
                    passphrase,
                    path: self.file_path.clone().expect("Button disabled if None"),
                    save,
                });
            }

            Msg::SendFileFinished => {
                log::info!("Finished sender worker");

                self.form_disabled = false;
            }
        }
    }
}
