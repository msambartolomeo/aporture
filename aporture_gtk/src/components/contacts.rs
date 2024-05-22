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
pub struct ContactsPage {
    contacts_ui: FactoryVecDeque<contact_row::Counter>,
    contacts: Option<Arc<RwLock<Contacts>>>,
    current_contact: String,
    sender_picker_dialog: Controller<OpenDialog>,
    // receiver_picker_dialog: Controller<OpenDialog>,
    aporture_dialog: Controller<Peer>,
}

#[derive(Debug)]
pub enum Msg {
    ContactsReady(Option<Arc<RwLock<Contacts>>>),
    SendFile(String),
    SenderPickerResponse(PathBuf),
    ReceiveFile(String),
    // ReceiverPickerResponse(PathBuf),
    PeerFinished,
    Ignore,
}

#[relm4::component(pub)]
impl SimpleComponent for ContactsPage {
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

        // let receiver_picker_dialog = OpenDialog::builder()
        //     .transient_for_native(&root)
        //     .launch(OpenDialogSettings {
        //         folder_mode: true,
        //         ..Default::default()
        //     })
        //     .forward(sender.input_sender(), |response| match response {
        //         OpenDialogResponse::Accept(path) => Msg::ReceiverPickerResponse(path),
        //         OpenDialogResponse::Cancel => Msg::Ignore,
        //     });

        let contacts_ui = FactoryVecDeque::builder()
            .launch(adw::PreferencesGroup::default())
            .forward(sender.input_sender(), |output| match output {
                contact_row::Output::Send(name) => Msg::SendFile(name),
                contact_row::Output::Receive(name) => Msg::ReceiveFile(name),
            });

        let model = Self {
            current_contact: "".to_string(),
            contacts_ui,
            contacts: None,
            sender_picker_dialog,
            // receiver_picker_dialog,
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
                    contacts.blocking_read().list().for_each(|(name, date)| {
                        let input = contact_row::Input {
                            name: name.clone(),
                            date: date.format("%d/%m/%Y %H:%M").to_string(),
                        };

                        contacts_ui.push_back(input);
                    })
                }
            }

            Msg::SendFile(name) => {
                self.current_contact = name;
                self.sender_picker_dialog.emit(OpenDialogMsg::Open)
            }

            Msg::SenderPickerResponse(path) => {
                let passphrase = PassphraseMethod::Contact(
                    self.current_contact.clone(),
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

            Msg::ReceiveFile(name) => {
                let passphrase = PassphraseMethod::Contact(
                    name,
                    self.contacts
                        .clone()
                        .expect("Method only called if contacts exists"),
                );

                log::info!("Starting sender worker");

                self.aporture_dialog.emit(peer::Msg::ReceiveFile {
                    passphrase,
                    destination: None,
                    save: None,
                })
            }

            // Msg::ReceiverPickerResponse(path) => {}
            Msg::PeerFinished => {
                log::info!("Finished sender worker");
            }

            Msg::Ignore => (),
        }
    }
}

mod contact_row {
    use adw::prelude::*;
    use relm4::prelude::*;
    use relm4_icons::icon_names;

    #[derive(Debug)]
    pub struct Counter {
        name: String,
        date: String,
    }

    #[derive(Debug)]
    pub enum Msg {
        SendFile,
        ReceiveFile,
    }

    #[derive(Debug)]
    pub struct Input {
        pub name: String,
        pub date: String,
    }

    #[derive(Debug)]
    pub enum Output {
        Send(String),
        Receive(String),
    }

    #[relm4::factory(pub)]
    impl FactoryComponent for Counter {
        type Init = Input;
        type Input = Msg;
        type Output = Output;
        type CommandOutput = ();
        type ParentWidget = adw::PreferencesGroup;

        view! {
            adw::ActionRow {
                set_title: &self.name,
                set_subtitle: &self.date,

                add_suffix = &gtk::Button {
                    set_icon_name: icon_names::SEND,

                    add_css_class: "flat",
                    add_css_class: "circular",

                    connect_clicked => Msg::SendFile,
                },

                add_suffix = &gtk::Button {
                    set_icon_name: icon_names::INBOX,

                    add_css_class: "flat",
                    add_css_class: "circular",

                    connect_clicked => Msg::ReceiveFile,
                },
            },
        }

        fn init_model(
            value: Self::Init,
            _index: &DynamicIndex,
            _sender: FactorySender<Self>,
        ) -> Self {
            Self {
                name: value.name,
                date: value.date,
            }
        }

        fn update(&mut self, msg: Self::Input, sender: FactorySender<Self>) {
            match msg {
                Msg::SendFile => sender.output(Output::Send(self.name.clone())).unwrap(),
                Msg::ReceiveFile => sender.output(Output::Receive(self.name.clone())).unwrap(),
            }
        }
    }
}
