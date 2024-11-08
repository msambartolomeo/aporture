use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::Arc;

use adw::prelude::*;
use relm4::prelude::*;
use relm4_components::open_dialog::{
    OpenDialog, OpenDialogMsg, OpenDialogResponse, OpenDialogSettings,
};
use relm4_icons::icon_names;
use tokio::sync::Mutex;

use crate::app;
use crate::components::dialog::peer::{self, ContactResult, Error, PassphraseMethod, Peer};
use aporture::fs::contacts::Contacts;
use aporture::passphrase;

const PASSPHRASE_WORD_COUNT: usize = 3;

#[derive(Debug)]
pub struct SenderPage {
    passphrase_entry: adw::EntryRow,
    file_entry: adw::ActionRow,
    save_contact: adw::SwitchRow,
    contact_entry: adw::EntryRow,
    passphrase_length: u32,
    file_path: Option<PathBuf>,
    file_picker_dialog: Controller<OpenDialog>,
    contacts: Option<Arc<Mutex<Contacts>>>,
    aporture_dialog: Controller<Peer>,
    form_disabled: bool,
}

#[derive(Debug)]
pub enum Msg {
    GeneratePassphrase,
    PassphraseChanged,
    FocusEditPassword,
    SaveContact,
    ContactsReady(Option<Arc<Mutex<Contacts>>>),
    FilePickerOpen,
    FilePickerResponse(PathBuf),
    SendFile,
    AportureFinished(Result<ContactResult, Error>),
    Ignore,
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

            #[local_ref]
            passphrase_entry -> adw::EntryRow {
                set_title: "Passphrase",
                set_text: &passphrase::generate(PASSPHRASE_WORD_COUNT),
                #[watch]
                set_sensitive: !model.form_disabled,

                add_css_class: "no-edit-button",

                set_can_focus: false,

                connect_changed => Msg::PassphraseChanged,

                add_suffix = &gtk::Button {
                    set_icon_name: icon_names::UPDATE,

                    add_css_class: "flat",
                    add_css_class: "circular",

                    connect_clicked => Msg::GeneratePassphrase,

                },

                add_suffix = &gtk::Button {
                    set_icon_name: icon_names::EDIT,

                    add_css_class: "flat",
                    add_css_class: "circular",

                    connect_clicked => Msg::FocusEditPassword,
                },
            },

            #[local_ref]
            file_path_entry -> adw::ActionRow {
                set_title: "File",

                add_suffix = &gtk::Button {
                    set_icon_name: icon_names::SEARCH_FOLDER,

                    add_css_class: "flat",
                    add_css_class: "circular",

                    connect_clicked => Msg::FilePickerOpen,
                },
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
        let file_picker_dialog = OpenDialog::builder()
            .transient_for_native(&root)
            .launch(OpenDialogSettings::default())
            .forward(sender.input_sender(), |response| match response {
                OpenDialogResponse::Accept(path) => Msg::FilePickerResponse(path),
                OpenDialogResponse::Cancel => Msg::Ignore,
            });

        let aporture_dialog = Peer::builder()
            .transient_for(&root)
            .launch(())
            .forward(sender.input_sender(), Msg::AportureFinished);

        let model = Self {
            passphrase_entry: adw::EntryRow::default(),
            file_entry: adw::ActionRow::default(),
            save_contact: adw::SwitchRow::default(),
            contact_entry: adw::EntryRow::default(),
            passphrase_length: 1,
            file_path: None,
            file_picker_dialog,
            contacts: None,
            aporture_dialog,
            form_disabled: false,
        };

        let passphrase_entry = &model.passphrase_entry;
        let file_path_entry = &model.file_entry;
        let save_contact = &model.save_contact;
        let contact_entry = &model.contact_entry;

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            Msg::GeneratePassphrase => self
                .passphrase_entry
                .set_text(&passphrase::generate(PASSPHRASE_WORD_COUNT)),

            Msg::PassphraseChanged => self.passphrase_length = self.passphrase_entry.text_length(),

            Msg::FocusEditPassword => {
                self.passphrase_entry.grab_focus_without_selecting();
            }

            Msg::SaveContact => {
                if self.contacts.is_none() && self.save_contact.is_active() {
                    sender
                        .output_sender()
                        .send(app::Request::Contacts)
                        .expect("Controller not dropped");
                }
            }

            Msg::ContactsReady(contacts) => {
                if self.contacts.is_none() {
                    if let Some(contacts) = contacts {
                        self.contacts = Some(contacts);
                    } else {
                        self.save_contact.set_active(false);
                    }
                }
            }

            Msg::FilePickerOpen => self.file_picker_dialog.emit(OpenDialogMsg::Open),

            Msg::FilePickerResponse(path) => {
                let name = path
                    .file_name()
                    .unwrap_or_else(|| OsStr::new("/"))
                    .to_string_lossy();
                self.file_entry.set_subtitle(&name);

                self.file_path = Some(path);
            }

            Msg::SendFile => {
                self.form_disabled = true;

                let passphrase = self.passphrase_entry.text().to_string();

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

                // TODO: MOVE TO AFTER RESULT

                sender
                    .output_sender()
                    .send(app::Request::Contacts)
                    .expect("Controller not dropped");
            }

            Msg::AportureFinished(result) => {
                log::info!("Finished sender worker");

                // TODO:
                match result {
                    Ok(ContactResult::Added) => sender
                        .output_sender()
                        .send(app::Request::Contacts)
                        .expect("Controller not dropped"),
                    Ok(ContactResult::PeerRefused) => todo!("Warning"),
                    Ok(ContactResult::NoOp) => {}
                    Err(_) => todo!("use error"),
                }

                self.form_disabled = false;
            }

            Msg::Ignore => (),
        }
    }
}
