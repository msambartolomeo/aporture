use std::sync::Arc;

use adw::prelude::*;
use relm4::prelude::*;
use tokio::sync::Mutex;

use aporture::fs::contacts::Contacts;

use crate::components::modal::utils::escape_action;
use crate::components::toaster::{Severity, Toaster};
use crate::emit;
#[derive(Debug)]
pub struct Holder {
    visible: bool,
    form_disabled: bool,
    contacts: Option<Arc<Mutex<Contacts>>>,
    exists_p_entry: adw::PasswordEntryRow,
    create_p_entry_1: adw::PasswordEntryRow,
    create_p_entry_2: adw::PasswordEntryRow,
    toaster: Toaster,
}

#[derive(Debug)]
pub enum Msg {
    Return,
    Get,
    Hide,
    Error(&'static str),
}

#[derive(Debug)]
pub enum Output {
    Cancel,
    Contacts(Arc<Mutex<Contacts>>),
    Error(&'static str),
}

#[relm4::component(pub)]
impl Component for Holder {
    type Init = ();
    type Input = Msg;
    type Output = Output;
    type CommandOutput = Option<Contacts>;

    view! {
        dialog = adw::Window {
            #[watch]
            set_visible: model.visible,
            set_modal: true,

            add_controller: escape_action!(Msg::Hide => sender),

            set_title: Some("Contacts"),

            grab_focus: (),

            set_default_width: 400,
            set_default_height: 450,

            adw::ToolbarView {
                set_top_bar_style: adw::ToolbarStyle::Raised,

                add_top_bar = &adw::HeaderBar { },

                #[local_ref]
                toaster -> adw::ToastOverlay {
                    if Contacts::exists() {
                        adw::PreferencesGroup {
                            set_margin_horizontal: 20,
                            set_margin_vertical: 50,

                            set_title: "Contacts",
                            set_description: Some("Enter password to access contacts"),

                            #[local_ref]
                            p -> adw::PasswordEntryRow {
                                set_title: "Password",

                                connect_entry_activated => Msg::Return,

                                #[watch]
                                set_sensitive: !model.form_disabled,
                            },

                            gtk::Button {
                                set_margin_all: 40,

                                add_css_class: "suggested-action",

                                set_label: "Enter",
                                connect_clicked => Msg::Return,
                            }
                        }
                    } else {
                        adw::PreferencesGroup {
                            set_margin_horizontal: 20,
                            set_margin_vertical: 50,

                            set_title: "Contacts",
                            set_description: Some("Enter password to encrypt contacts database"),

                            #[local_ref]
                            p1 -> adw::PasswordEntryRow {
                                set_title: "Password",
                                #[watch]
                                set_sensitive: !model.form_disabled,
                            },

                            #[local_ref]
                            p2 -> adw::PasswordEntryRow {
                                set_title: "Repeat Password",

                                connect_entry_activated => Msg::Return,

                                #[watch]
                                set_sensitive: !model.form_disabled,
                            },

                            gtk::Button {
                                set_margin_all: 40,

                                add_css_class: "suggested-action",

                                set_label: "Enter",
                                connect_clicked => Msg::Return,
                            }
                        }
                    },
                }
            },

            connect_close_request[sender] => move |_| {
                sender.input(Msg::Hide);
                gtk::glib::Propagation::Stop
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {
            toaster: Toaster::default(),
            visible: false,
            form_disabled: false,
            contacts: None,
            exists_p_entry: adw::PasswordEntryRow::new(),
            create_p_entry_1: adw::PasswordEntryRow::new(),
            create_p_entry_2: adw::PasswordEntryRow::new(),
        };

        let p = &model.exists_p_entry;
        let p1 = &model.create_p_entry_1;
        let p2 = &model.create_p_entry_2;
        let toaster = model.toaster.as_ref();

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _: &Self::Root) {
        match msg {
            Msg::Return => {
                if self.contacts.is_none() {
                    if Contacts::exists() {
                        self.exists_p_entry.remove_css_class("error");
                        let password = self.exists_p_entry.text();

                        sender.oneshot_command(async move {
                            Contacts::load(&password.into_bytes()).await.ok()
                        });
                    } else {
                        let p1 = self.create_p_entry_1.text();
                        let p2 = self.create_p_entry_2.text();

                        if p1 == p2 {
                            self.create_p_entry_1.remove_css_class("error");
                            self.create_p_entry_2.remove_css_class("error");

                            sender.oneshot_command(async move {
                                Contacts::empty(&p1.into_bytes()).await.ok()
                            });
                        } else {
                            sender.input(Msg::Error("The passwords do not match"));
                            self.create_p_entry_1.add_css_class("error");
                            self.create_p_entry_2.add_css_class("error");
                        }
                    }
                }
            }

            Msg::Get => {
                if let Some(contacts) = &self.contacts {
                    emit!(Output::Contacts(contacts.clone()) => sender);
                } else {
                    self.visible = true;
                }
            }

            Msg::Hide => {
                emit!(Output::Cancel => sender);
                self.visible = false;
            }

            Msg::Error(msg) => self.toaster.add_toast(msg, Severity::Error),
        }
    }

    fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        sender: ComponentSender<Self>,
        _: &Self::Root,
    ) {
        if message.is_none() {
            if Contacts::exists() {
                self.exists_p_entry.add_css_class("error");
            } else {
                self.create_p_entry_1.add_css_class("error");
                self.create_p_entry_2.add_css_class("error");
            }
            sender.input(Msg::Error("Wrong password, try again"));
        } else {
            self.contacts = message.map(|c| Arc::new(Mutex::new(c)));

            match self.contacts {
                Some(ref contacts) => emit!(Output::Contacts(contacts.clone()) => sender),
                None => emit!(Output::Error("Could not load contacts") => sender),
            }

            self.visible = false;
        }
    }
}
