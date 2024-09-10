use std::sync::Arc;

use adw::prelude::*;
use relm4::prelude::*;
use tokio::sync::RwLock;

use aporture::fs::contacts::Contacts;

#[derive(Debug)]
pub struct Holder {
    visible: bool,
    form_disabled: bool,
    contacts: Option<Arc<RwLock<Contacts>>>,
    exists_p_entry: adw::PasswordEntryRow,
    create_p_entry_1: adw::PasswordEntryRow,
    create_p_entry_2: adw::PasswordEntryRow,
}

#[derive(Debug)]
pub enum Msg {
    Return,
    Get,
    Hide,
}

#[relm4::component(pub)]
impl Component for Holder {
    type Init = ();
    type Input = Msg;
    type Output = Option<Arc<RwLock<Contacts>>>;
    // TODO: Handle error with error messages
    type CommandOutput = Option<Contacts>;

    view! {
        dialog = adw::Window {
            #[watch]
            set_visible: model.visible,
            set_modal: true,

            set_title: Some("Contacts"),

            set_default_width: 400,
            set_default_height: 500,

            adw::ToolbarView {
                set_top_bar_style: adw::ToolbarStyle::Raised,

                add_top_bar = &adw::HeaderBar { },

                if Contacts::exists() {
                    adw::PreferencesGroup {
                        set_margin_horizontal: 20,
                        set_margin_vertical: 50,

                        set_title: "Contacts",
                        set_description: Some("Enter password to access contacts"),

                        #[local_ref]
                        p -> adw::PasswordEntryRow {
                            set_title: "Password",
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

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _: &Self::Root) {
        self.visible = true;

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
                            // TODO: Error message
                            self.create_p_entry_1.add_css_class("error");
                            self.create_p_entry_2.add_css_class("error");
                        }
                    }
                }
            }
            Msg::Get => {
                let Some(contacts) = &self.contacts else {
                    return;
                };

                sender
                    .output(Some(contacts.clone()))
                    .expect("Component must not be dropped");
            }

            Msg::Hide => {
                sender.output(None).expect("Component must not be dropped");
                self.visible = false;
            }
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
        } else {
            self.contacts = message.map(|c| Arc::new(RwLock::new(c)));

            sender
                .output(self.contacts.clone())
                .expect("Component not dropped");

            self.visible = false;
        }
    }
}
