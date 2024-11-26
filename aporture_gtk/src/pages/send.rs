use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::Arc;

use adw::prelude::*;
use gtk::gdk::Display;
use open_dialog::{OpenDialog, OpenDialogMsg, OpenDialogResponse, OpenDialogSettings};
use relm4::prelude::*;
use relm4_components::open_dialog;
use relm4_icons::icon_names;
use tokio::sync::Mutex;

use aporture::fs::contacts::Contacts;
use aporture::passphrase;

use crate::components::modal::aporture::{ContactAction, Params, PassphraseMethod, Peer};
use crate::components::modal::aporture::{Error as AportureError, Msg as AportureMsg};
use crate::components::toaster::Severity;
use crate::{app, emit};

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
    directory_picker_dialog: Controller<OpenDialog>,
    contacts: Option<Arc<Mutex<Contacts>>>,
    aporture_dialog: Controller<Peer>,
    form_disabled: bool,
}

#[derive(Debug)]
pub enum Msg {
    GeneratePassphrase,
    PassphraseChanged,
    FocusEditPassword,
    CopyPassword,
    SaveContact,
    ContactsReady(Option<Arc<Mutex<Contacts>>>),
    FilePickerOpen,
    FilePickerResponse(PathBuf),
    SendFile,
    AportureFinished(Result<ContactAction, AportureError>),
    Ignore,
    DirectoryPickerOpen,
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

                #[name = "random"]
                add_suffix = &gtk::Button {
                    set_icon_name: icon_names::UPDATE,

                    set_tooltip_text: Some("Generate passphrase"),

                    add_css_class: "flat",
                    add_css_class: "circular",

                    connect_clicked => Msg::GeneratePassphrase,

                },

                #[name = "edit"]
                add_suffix = &gtk::Button {
                    set_icon_name: icon_names::EDIT,

                    set_tooltip_text: Some("Edit manually"),

                    add_css_class: "flat",
                    add_css_class: "circular",

                    connect_clicked => Msg::FocusEditPassword,
                },

                #[name = "copy"]
                add_suffix = &gtk::Button {
                    set_icon_name: icon_names::COPY,

                    set_tooltip_text: Some("Copy"),

                    add_css_class: "flat",
                    add_css_class: "circular",

                    connect_clicked => Msg::CopyPassword,
                },
            },

            #[local_ref]
            file_path_entry -> adw::ActionRow {
                set_title: "File",
                #[watch]
                set_subtitle: &model.file_path.as_ref().map_or("Select file to send".to_owned(), |p| p.display().to_string()),

                add_suffix = &gtk::Button {
                    set_icon_name: icon_names::EDIT_FIND,

                    set_tooltip_text: Some("Select file"),

                    add_css_class: "flat",
                    add_css_class: "circular",

                    connect_clicked => Msg::DirectoryPickerOpen,
                },

                add_suffix = &gtk::Button {
                    set_icon_name: icon_names::SEARCH_FOLDER,

                    set_tooltip_text: Some("Select folder"),

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
            passphrase_length: 1,
            file_path: None,
            file_picker_dialog,
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
            Msg::GeneratePassphrase => self
                .passphrase_entry
                .set_text(&passphrase::generate(PASSPHRASE_WORD_COUNT)),

            Msg::PassphraseChanged => self.passphrase_length = self.passphrase_entry.text_length(),

            Msg::FocusEditPassword => {
                self.passphrase_entry.grab_focus_without_selecting();
            }

            Msg::CopyPassword => {
                let Some(clipboard) = Display::default().as_ref().map(DisplayExt::clipboard) else {
                    emit!(app::Request::ToastS("Could not copy to clipboard", Severity::Error) => sender);

                    return;
                };

                let text = self.passphrase_entry.text().to_string();

                clipboard.set_text(&text);

                emit!(app::Request::ToastS("Coppied passphrase to clipboard", Severity::Info) => sender);
            }

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

            Msg::FilePickerOpen => self.file_picker_dialog.emit(OpenDialogMsg::Open),

            Msg::DirectoryPickerOpen => self.directory_picker_dialog.emit(OpenDialogMsg::Open),

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
                    let contact = self.contact_entry.text().to_string();
                    let contacts = self
                        .contacts
                        .clone()
                        .expect("Should be loaded as save contact is true");

                    (contact, contacts)
                });
                let path = self
                    .file_path
                    .clone()
                    .expect("Should have file to be able to call send");

                log::info!("Starting sender worker");

                self.aporture_dialog
                    .emit(AportureMsg::SendFile(Params::new(passphrase, path, save)));
            }

            Msg::AportureFinished(result) => {
                log::info!("Finished sender worker");

                match result {
                    Ok(ContactAction::Added) => emit!(app::Request::Contacts => sender),
                    Ok(ContactAction::PeerRefused) => {
                        emit!(app::Request::ToastS("Peer refused to save contact", Severity::Warn) => sender);
                    }
                    Ok(ContactAction::NoOp) => {}
                    Err(e) => emit!(app::Request::Toast(e.to_string(), Severity::Error) => sender),
                }

                self.form_disabled = false;
            }

            Msg::Ignore => (),
        }
    }
}
