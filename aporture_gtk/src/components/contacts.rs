use std::path::PathBuf;
use std::sync::Arc;

use adw::prelude::*;
use relm4::factory::FactoryVecDeque;
use relm4::prelude::*;
use relm4_components::open_dialog::{
    OpenDialog, OpenDialogMsg, OpenDialogResponse, OpenDialogSettings,
};
use tokio::sync::RwLock;

use crate::app;
use crate::components::dialog::peer::{self, PassphraseMethod, Peer};
use aporture::fs::contacts::Contacts;

#[derive(Debug)]
pub struct ContactPage {
    contacts_ui: FactoryVecDeque<contact_row::Contact>,
    contacts: Option<Arc<RwLock<Contacts>>>,
    current_picking_index: usize,
    sender_picker_dialog: Controller<OpenDialog>,
    receiver_picker_dialog: Controller<OpenDialog>,
    aporture_dialog: Controller<Peer>,
}

#[derive(Debug)]
pub enum Msg {
    ContactsReady(Option<Arc<RwLock<Contacts>>>),
    SendFile(String, PathBuf),
    SenderPickerOpen(usize),
    SenderPickerResponse(PathBuf),
    ReceiveFile(String, PathBuf),
    ReceiverPickerOpen(usize),
    ReceiverPickerResponse(PathBuf),
    PeerFinished,
    Ignore,
}

#[relm4::component(pub)]
impl SimpleComponent for ContactPage {
    type Init = ();
    type Input = Msg;
    type Output = app::Request;

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
        let aporture_dialog = Peer::builder()
            .transient_for(&root)
            .launch(())
            .forward(sender.input_sender(), |_| Msg::PeerFinished); // TODO: Handle Errors

        let sender_picker_dialog = OpenDialog::builder()
            .transient_for_native(&root)
            .launch(OpenDialogSettings::default())
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

        let contacts_ui = FactoryVecDeque::builder()
            .launch(adw::PreferencesGroup::default())
            .forward(sender.input_sender(), Msg::from);

        let model = Self {
            current_picking_index: 0,
            contacts_ui,
            contacts: None,
            sender_picker_dialog,
            receiver_picker_dialog,
            aporture_dialog,
        };

        let contacts_box = model.contacts_ui.widget();

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            Msg::ContactsReady(contacts) => {
                self.contacts = contacts;
                if let Some(contacts) = &self.contacts {
                    let mut contacts_ui = self.contacts_ui.guard();
                    contacts_ui.clear();
                    contacts.blocking_read().list().for_each(|(name, date)| {
                        let input = contact_row::Input {
                            name: name.clone(),
                            date: date.format("%d/%m/%Y %H:%M").to_string(),
                        };

                        contacts_ui.push_back(input);
                    });
                }
            }

            Msg::SendFile(name, path) => {
                let passphrase = PassphraseMethod::Contact(
                    name,
                    self.contacts
                        .clone()
                        .expect("Method only called if contacts exists"),
                );

                log::info!("Starting sender worker");

                self.aporture_dialog.emit(peer::Msg::SendFile {
                    passphrase,
                    path,
                    save: None,
                });
            }

            Msg::ReceiveFile(name, path) => {
                let passphrase = PassphraseMethod::Contact(
                    name,
                    self.contacts
                        .clone()
                        .expect("Method only called if contacts exists"),
                );

                log::info!("Starting sender worker");

                self.aporture_dialog.emit(peer::Msg::ReceiveFile {
                    passphrase,
                    destination: Some(path),
                    save: None,
                });
            }

            Msg::SenderPickerOpen(index) => {
                self.current_picking_index = index;

                self.sender_picker_dialog.emit(OpenDialogMsg::Open);
            }

            Msg::SenderPickerResponse(path) => {
                use contact_row::Msg as ContactMsg;

                self.contacts_ui.send(
                    self.current_picking_index,
                    ContactMsg::SendFilePickerClosed(path),
                );
            }

            Msg::ReceiverPickerOpen(index) => {
                self.current_picking_index = index;

                self.receiver_picker_dialog.emit(OpenDialogMsg::Open);
            }

            Msg::ReceiverPickerResponse(path) => {
                use contact_row::Msg as ContactMsg;

                self.contacts_ui.send(
                    self.current_picking_index,
                    ContactMsg::ReceiveFilePickerClosed(path),
                );
            }

            Msg::PeerFinished => {
                log::info!("Finished sender worker");
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

    #[derive(Debug)]
    pub struct Contact {
        name: String,
        date: String,
        path: Option<PathBuf>,
        destination: PathBuf,
        index: DynamicIndex,
    }

    #[derive(Debug)]
    pub enum Msg {
        SendFilePickerOpen,
        SendFilePickerClosed(PathBuf),
        SendFile,
        ReceiveFilePickerOpen,
        ReceiveFilePickerClosed(PathBuf),
        ReceiveFile,
    }

    #[derive(Debug)]
    pub struct Input {
        pub name: String,
        pub date: String,
    }

    #[derive(Debug)]
    pub enum Output {
        Send(String, PathBuf),
        SendFilePicker(usize),
        ReceiveFilePicker(usize),
        Receive(String, PathBuf),
    }

    #[relm4::factory(pub)]
    impl FactoryComponent for Contact {
        type Init = Input;
        type Input = Msg;
        type Output = Output;
        type CommandOutput = ();
        type ParentWidget = adw::PreferencesGroup;

        view! {
            adw::ExpanderRow {
                set_title: &self.name,
                set_subtitle: &self.date,

                add_row = &adw::ActionRow {
                    set_title: "Send",
                    #[watch]
                    set_subtitle: &self.path.as_ref().map_or("Select file to send".to_owned(), |p| p.display().to_string()),

                    add_suffix = &gtk::Button {
                        set_icon_name: icon_names::SEARCH_FOLDER,

                        add_css_class: "flat",
                        add_css_class: "circular",

                        connect_clicked => Msg::SendFilePickerOpen,
                    },

                    add_suffix = &gtk::Button {
                        set_icon_name: icon_names::SEND,

                        add_css_class: "flat",
                        add_css_class: "circular",

                        connect_clicked => Msg::SendFile,
                    },
                },

                add_row = &adw::ActionRow {
                    set_title: "Receive",
                    #[watch]
                    set_subtitle: &self.destination.display().to_string(),

                    add_suffix = &gtk::Button {
                        set_icon_name: icon_names::SEARCH_FOLDER,

                        add_css_class: "flat",
                        add_css_class: "circular",

                        connect_clicked => Msg::ReceiveFilePickerOpen,
                    },

                    add_suffix = &gtk::Button {
                        set_icon_name: icon_names::INBOX,

                        add_css_class: "flat",
                        add_css_class: "circular",

                        connect_clicked => Msg::ReceiveFile,
                    },
                },
            }
        }

        fn init_model(
            value: Self::Init,
            index: &DynamicIndex,
            _sender: FactorySender<Self>,
        ) -> Self {
            Self {
                name: value.name,
                date: value.date,
                index: index.clone(),
                path: None,
                destination: aporture::fs::downloads_directory().expect("Valid download dir"),
            }
        }

        fn update(&mut self, msg: Self::Input, sender: FactorySender<Self>) {
            match msg {
                Msg::SendFile => {
                    if let Some(path) = self.path.clone() {
                        sender
                            .output(Output::Send(self.name.clone(), path))
                            .expect("Not dropped");
                    } else {
                        sender.input(Msg::SendFilePickerOpen);
                    }
                }

                Msg::ReceiveFile => sender
                    .output(Output::Receive(self.name.clone(), self.destination.clone()))
                    .expect("Not dropped"),
                Msg::SendFilePickerOpen => sender
                    .output(Output::SendFilePicker(self.index.current_index()))
                    .expect("Not dropped"),
                Msg::ReceiveFilePickerOpen => sender
                    .output(Output::ReceiveFilePicker(self.index.current_index()))
                    .expect("Not dropped"),
                Msg::SendFilePickerClosed(path) => self.path = Some(path),
                Msg::ReceiveFilePickerClosed(path) => self.destination = path,
            }
        }
    }

    impl From<Output> for super::Msg {
        fn from(output: Output) -> Self {
            match output {
                Output::Send(name, path) => Self::SendFile(name, path),
                Output::Receive(name, path) => Self::ReceiveFile(name, path),
                Output::SendFilePicker(index) => Self::SenderPickerOpen(index),
                Output::ReceiveFilePicker(index) => Self::ReceiverPickerOpen(index),
            }
        }
    }
}
