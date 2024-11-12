use std::path::PathBuf;
use std::sync::Arc;

use adw::prelude::*;
use open_dialog::{OpenDialog, OpenDialogMsg, OpenDialogResponse, OpenDialogSettings};
use relm4::prelude::*;
use relm4_components::open_dialog;
use relm4_icons::icon_names;
use tokio::sync::Mutex;

use aporture::fs::contacts::Contacts;

use crate::components::modal::aporture::{ContactResult, PassphraseMethod, Peer};
use crate::components::modal::aporture::{Error as AportureError, Msg as AportureMsg};
use crate::components::toaster::Severity;
use crate::{app, emit};

#[derive(Debug)]
pub struct ReceiverPage {
    passphrase_entry: adw::EntryRow,
    file_entry: adw::ActionRow,
    save_contact: adw::SwitchRow,
    contact_entry: adw::EntryRow,
    passphrase_length: u32,
    destination: Option<PathBuf>,
    directory_picker_dialog: Controller<OpenDialog>,
    contacts: Option<Arc<Mutex<Contacts>>>,
    aporture_dialog: Controller<Peer>,
    form_disabled: bool,
}

#[derive(Debug)]
pub enum Msg {
    ReceiveFile,
    AportureFinished(Result<ContactResult, AportureError>),
    PassphraseChanged,
    SaveContact,
    ContactsReady(Option<Arc<Mutex<Contacts>>>),
    FilePickerOpen,
    FilePickerResponse(PathBuf),
    Ignore,
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
                add_css_class: "suggested-action",

                set_label: "Connect",
                #[watch]
                set_sensitive: !model.form_disabled && model.passphrase_length != 0,

                connect_clicked => Msg::ReceiveFile,
            },

            #[local_ref]
            passphrase_entry -> adw::EntryRow {
                set_title: "Passphrase",
                #[watch]
                set_sensitive: !model.form_disabled,

                connect_changed => Msg::PassphraseChanged,
            },

            #[local_ref]
            file_path_entry -> adw::ActionRow {
                set_title: "Destination",
                #[watch]
                set_subtitle: &model.destination.as_ref().map_or("Select destination to receive into".to_owned(), |p| p.display().to_string()),

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
        let directory_picker_dialog = OpenDialog::builder()
            .transient_for_native(&root)
            .launch(OpenDialogSettings {
                folder_mode: true,
                ..Default::default()
            })
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
            passphrase_length: 0,
            destination: aporture::fs::downloads_directory(),
            directory_picker_dialog,
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
            Msg::ReceiveFile => {
                self.form_disabled = true;

                let passphrase = self.passphrase_entry.text().to_string();

                log::info!("Selected passphrase is {}", passphrase);

                let passphrase = PassphraseMethod::Direct(passphrase.into_bytes());
                let save = self.save_contact.is_active().then(|| {
                    let contact = self.contact_entry.text().to_string();
                    let contacts = self
                        .contacts
                        .clone()
                        .expect("Should be loaded as save contact is true");

                    (contact, contacts)
                });
                let destination = self
                    .destination
                    .clone()
                    .expect("Should have destination to be able to call send");

                log::info!("Starting receiver worker");

                self.aporture_dialog.emit(AportureMsg::ReceiveFile {
                    passphrase,
                    destination,
                    save,
                });
            }

            Msg::AportureFinished(result) => {
                log::info!("Finished receiver worker");

                match result {
                    Ok(ContactResult::Added) => emit!(app::Request::Contacts => sender),
                    Ok(ContactResult::PeerRefused) => {
                        emit!(app::Request::ToastS("Peer refused to save contact", Severity::Warn) => sender);
                    }
                    Ok(ContactResult::NoOp) => {}
                    Err(e) => {
                        emit!(app::Request::Toast(e.to_string(), Severity::Error) => sender);
                    }
                }

                self.form_disabled = false;
            }

            Msg::PassphraseChanged => self.passphrase_length = self.passphrase_entry.text_length(),

            Msg::SaveContact => {
                if self.contacts.is_none() && self.save_contact.is_active() {
                    emit!(app::Request::Contacts => sender);
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

            Msg::FilePickerOpen => self.directory_picker_dialog.emit(OpenDialogMsg::Open),

            Msg::FilePickerResponse(path) => {
                self.file_entry.set_subtitle(&path.to_string_lossy());

                self.destination = Some(path);
            }

            Msg::Ignore => (),
        }
    }
}
