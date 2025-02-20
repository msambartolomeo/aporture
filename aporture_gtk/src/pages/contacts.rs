use std::hash::RandomState;
use std::path::PathBuf;
use std::sync::Arc;

use adw::prelude::*;
use open_dialog::{OpenDialog, OpenDialogMsg, OpenDialogResponse, OpenDialogSettings};
use relm4::factory::FactoryHashMap;
use relm4::prelude::*;
use relm4_components::open_dialog;
use tokio::sync::Mutex;

use crate::components::confirmation::Confirmation;
use crate::components::file_chooser;
use crate::components::modal::aporture::{ContactAction, Params, PassphraseMethod, Peer};
use crate::components::modal::aporture::{Error as AportureError, TransferType};
use crate::components::toaster::Severity;
use crate::{app, emit};

use aporture::fs::contacts::Contacts;

#[derive(Debug)]
pub struct ContactPage {
    contacts_ui: FactoryHashMap<String, contact_row::Contact, RandomState>,
    contacts: Option<Arc<Mutex<Contacts>>>,
    current_contact: String,
    sender_picker_dialog: Controller<OpenDialog>,
    sender_dir_picker_dialog: Controller<OpenDialog>,
    receiver_picker_dialog: Controller<OpenDialog>,
    peer: Option<Controller<Peer>>,
}

impl ContactPage {
    fn contacts(&self) -> Arc<Mutex<Contacts>> {
        self.contacts
            .clone()
            .expect("Contacts must be present for ContactPage to be shown")
    }
}

#[derive(Debug)]
pub enum Msg {
    ContactsReady(Option<Arc<Mutex<Contacts>>>),
    SendFile(String, PathBuf),
    SenderPickerOpen(String),
    SenderPickerResponse(PathBuf),
    ReceiveFile(String, PathBuf),
    ReceiverPickerOpen(String),
    ReceiverPickerResponse(PathBuf),
    DeleteContact(String),
    DeleteContactUI(String),
    AportureFinished(Result<ContactAction, AportureError>),
    Ignore,
}

#[relm4::component(pub)]
impl Component for ContactPage {
    type Init = ();
    type Input = Msg;
    type Output = app::Request;
    type CommandOutput = ();

    view! {
        gtk::ScrolledWindow {
            set_hscrollbar_policy: gtk::PolicyType::Never,
            set_min_content_height: 500,
            set_vexpand: true,

            #[local_ref]
            contacts_box -> adw::PreferencesGroup {
                set_margin_horizontal: 20,
                set_margin_vertical: 50,

                set_title: "Contacts",
                set_description: Some("Choose a registered contact to send or receive files"),
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let sender_picker_dialog = OpenDialog::builder()
            .transient_for_native(&root)
            .launch(OpenDialogSettings::default())
            .forward(sender.input_sender(), |response| match response {
                OpenDialogResponse::Accept(path) => Msg::SenderPickerResponse(path),
                OpenDialogResponse::Cancel => Msg::Ignore,
            });

        let sender_dir_picker_dialog = OpenDialog::builder()
            .transient_for_native(&root)
            .launch(OpenDialogSettings {
                folder_mode: true,
                ..Default::default()
            })
            .forward(sender.input_sender(), |response| match response {
                OpenDialogResponse::Accept(path) => Msg::SenderPickerResponse(path),
                OpenDialogResponse::Cancel => Msg::Ignore,
            });

        let receiver_picker_dialog = OpenDialog::builder()
            .transient_for_native(&root)
            .launch(OpenDialogSettings {
                folder_mode: true,
                ..Default::default()
            })
            .forward(sender.input_sender(), |response| match response {
                OpenDialogResponse::Accept(path) => Msg::ReceiverPickerResponse(path),
                OpenDialogResponse::Cancel => Msg::Ignore,
            });

        let contacts_ui = FactoryHashMap::builder()
            .launch(adw::PreferencesGroup::default())
            .forward(sender.input_sender(), Msg::from);

        let model = Self {
            current_contact: String::default(),
            contacts_ui,
            contacts: None,
            sender_picker_dialog,
            sender_dir_picker_dialog,
            receiver_picker_dialog,
            peer: None,
        };

        let contacts_box = model.contacts_ui.widget();

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    #[allow(clippy::too_many_lines)]
    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, root: &Self::Root) {
        match msg {
            Msg::ContactsReady(contacts) => {
                if let Some(contacts) = contacts {
                    let contacts = self.contacts.get_or_insert(contacts);

                    self.contacts_ui.clear();

                    let destination = aporture::fs::downloads_directory();

                    if destination.is_none() {
                        let toast = app::Request::ToastS(
                            "Could not find default receive directory, please pick one before sending",
                            Severity::Warn,
                        );

                        emit!(toast => sender);
                    }

                    contacts.blocking_lock().list().for_each(|(name, date)| {
                        let data = contact_row::Input {
                            date: date.format("%d/%m/%Y %H:%M").to_string(),
                            destination: destination.clone(),
                        };

                        self.contacts_ui.insert(name.clone(), data);
                    });
                }
            }

            Msg::SendFile(name, path) => {
                let passphrase = PassphraseMethod::Contact(name, self.contacts());

                log::info!("Starting sender worker");

                let controller = Peer::builder()
                    .transient_for(root)
                    .launch(TransferType::Send(Params::new(passphrase, path, None)))
                    .forward(sender.input_sender(), Msg::AportureFinished);

                self.peer = Some(controller);
            }

            Msg::ReceiveFile(name, path) => {
                let passphrase = PassphraseMethod::Contact(name, self.contacts());

                log::info!("Starting sender worker");

                let controller = Peer::builder()
                    .transient_for(root)
                    .launch(TransferType::Receive(Params::new(passphrase, path, None)))
                    .forward(sender.input_sender(), Msg::AportureFinished);

                self.peer = Some(controller);
            }

            Msg::SenderPickerOpen(index) => {
                self.current_contact = index;

                file_chooser::choose(
                    root,
                    self.sender_picker_dialog.sender().clone(),
                    self.sender_dir_picker_dialog.sender().clone(),
                );
            }

            Msg::SenderPickerResponse(path) => {
                use contact_row::Msg as ContactMsg;

                self.contacts_ui.send(
                    &self.current_contact,
                    ContactMsg::SendFilePickerClosed(path),
                );
            }

            Msg::ReceiverPickerOpen(index) => {
                self.current_contact = index;

                self.receiver_picker_dialog.emit(OpenDialogMsg::Open);
            }

            Msg::ReceiverPickerResponse(path) => {
                use contact_row::Msg as ContactMsg;

                self.contacts_ui.send(
                    &self.current_contact,
                    ContactMsg::ReceiveFilePickerClosed(path),
                );
            }

            Msg::DeleteContact(contact) => {
                let message = format!("delete contact \"{contact}\"");
                let contacts = self.contacts();

                Confirmation::new(&message)
                    .confirm("Delete")
                    .deny("Cancel")
                    .choose(root, move || {
                        let mut contacts = contacts.blocking_lock();

                        contacts.delete(&contact);

                        match contacts.save_blocking() {
                            Ok(()) => sender.input(Msg::DeleteContactUI(contact)),
                            Err(_) => emit!(app::Request::ToastS("Could not delete contact", Severity::Warn) => sender),
                        }
                    });
            }

            Msg::DeleteContactUI(contact) => {
                self.contacts_ui.remove(&contact);
            }

            Msg::AportureFinished(result) => {
                log::info!("Finished contact worker");

                drop(self.peer.take());

                if result.is_ok() {
                    emit!(app::Request::ToastS("Transfer completed!", Severity::Success) => sender);
                }

                match result {
                    Ok(ContactAction::Added) => emit!(app::Request::Contacts => sender),
                    Ok(ContactAction::PeerRefused) => {
                        emit!(app::Request::ToastS("Peer refused to save contact", Severity::Warn) => sender);
                    }
                    Ok(ContactAction::NoOp) => {}
                    Err(e @ AportureError::Cancel) => {
                        emit!(app::Request::Toast(e.to_string(), Severity::Warn) => sender);
                    }
                    Err(e) => emit!(app::Request::Toast(e.to_string(), Severity::Error) => sender),
                }
            }

            Msg::Ignore => (),
        }
    }
}

mod contact_row {
    use std::path::PathBuf;

    use adw::prelude::*;
    use relm4::prelude::*;
    use relm4_icons::icon_names;

    use crate::emit;

    #[derive(Debug)]
    pub struct Contact {
        name: String,
        date: String,
        path: Option<PathBuf>,
        destination: Option<PathBuf>,
        expanded: bool,
    }

    #[derive(Debug)]
    pub enum Msg {
        SendFilePickerOpen,
        SendFilePickerClosed(PathBuf),
        SendFile,
        ReceiveFilePickerOpen,
        ReceiveFilePickerClosed(PathBuf),
        ReceiveFile,
        Delete,
        Expand,
    }

    #[derive(Debug)]
    pub struct Input {
        pub date: String,
        pub destination: Option<PathBuf>,
    }

    #[derive(Debug)]
    pub enum Output {
        Send(String, PathBuf),
        SendFilePicker(String),
        ReceiveFilePicker(String),
        Receive(String, PathBuf),
        Delete(String),
    }

    #[relm4::factory(pub)]
    impl FactoryComponent for Contact {
        type Init = Input;
        type Input = Msg;
        type Output = Output;
        type CommandOutput = ();
        type ParentWidget = adw::PreferencesGroup;
        type Index = String;

        view! {
            #[name = "expander"]
            adw::ExpanderRow {
                set_title: &self.name,
                set_subtitle: &self.date,

                connect_expanded_notify => Msg::Expand,

                add_suffix = &gtk::Button {
                    // NOTE: BROKEN set_icon_name: icon_names::USER_TRASH,
                    set_icon_name: "user-trash-symbolic",

                    set_tooltip_text: Some("Delete contact"),

                    add_css_class: "flat",
                    add_css_class: "circular",

                    #[watch]
                    set_visible: self.expanded,

                    connect_clicked => Msg::Delete,
                },

                add_row = &adw::ActionRow {
                    set_title: "Send",
                    #[watch]
                    set_subtitle: &self.path.as_ref().map_or("Select file to send".to_owned(), |p| p.display().to_string()),

                    add_suffix = &gtk::Button {
                        set_icon_name: icon_names::SEARCH_FOLDER,

                        set_tooltip_text: Some("Select files to send"),

                        add_css_class: "flat",
                        add_css_class: "circular",

                        connect_clicked => Msg::SendFilePickerOpen,
                    },

                    add_suffix = &gtk::Button {
                        set_icon_name: icon_names::SEND,

                        set_tooltip_text: Some("Send file"),

                        add_css_class: "flat",
                        add_css_class: "circular",

                        connect_clicked => Msg::SendFile,
                    },
                },

                add_row = &adw::ActionRow {
                    set_title: "Receive",
                    #[watch]
                    set_subtitle: &self.destination.as_ref().map_or("Select destination to receive into".to_owned(), |p| p.display().to_string()),

                    add_suffix = &gtk::Button {
                        set_icon_name: icon_names::SEARCH_FOLDER,

                        set_tooltip_text: Some("Select destination"),

                        add_css_class: "flat",
                        add_css_class: "circular",

                        connect_clicked => Msg::ReceiveFilePickerOpen,
                    },

                    add_suffix = &gtk::Button {
                        set_icon_name: icon_names::INBOX,

                        set_tooltip_text: Some("Receive file"),

                        add_css_class: "flat",
                        add_css_class: "circular",

                        connect_clicked => Msg::ReceiveFile,
                    },
                },
            }
        }

        fn init_model(value: Self::Init, index: &String, _sender: FactorySender<Self>) -> Self {
            Self {
                expanded: false,
                name: index.clone(),
                date: value.date,
                destination: value.destination,
                path: None,
            }
        }

        fn update(&mut self, msg: Self::Input, sender: FactorySender<Self>) {
            match msg {
                Msg::SendFile => {
                    if let Some(path) = self.path.clone() {
                        emit!(Output::Send(self.name.clone(), path) => sender);
                    } else {
                        sender.input(Msg::SendFilePickerOpen);
                    }
                }

                Msg::ReceiveFile => {
                    if let Some(path) = self.destination.clone() {
                        emit!(Output::Receive(self.name.clone(), path) => sender);
                    } else {
                        sender.input(Msg::ReceiveFilePickerOpen);
                    }
                }

                Msg::SendFilePickerOpen => {
                    emit!(Output::SendFilePicker(self.name.clone()) => sender);
                }

                Msg::ReceiveFilePickerOpen => {
                    emit!(Output::ReceiveFilePicker(self.name.clone()) => sender);
                }

                Msg::SendFilePickerClosed(path) => self.path = Some(path),

                Msg::ReceiveFilePickerClosed(path) => self.destination = Some(path),

                Msg::Expand => self.expanded = !self.expanded,

                Msg::Delete => emit!(Output::Delete(self.name.clone()) => sender),
            }
        }
    }

    impl From<Output> for super::Msg {
        fn from(output: Output) -> Self {
            match output {
                Output::Send(name, path) => Self::SendFile(name, path),
                Output::Receive(name, path) => Self::ReceiveFile(name, path),
                Output::SendFilePicker(name) => Self::SenderPickerOpen(name),
                Output::ReceiveFilePicker(name) => Self::ReceiverPickerOpen(name),
                Output::Delete(name) => Self::DeleteContact(name),
            }
        }
    }
}
