use std::net::SocketAddr;

use adw::prelude::*;
use aporture::fs::config::Config;
use relm4::prelude::*;
use relm4_icons::icon_names;

use crate::components::modal::utils::escape_action;
use crate::components::toaster::{Severity, Toaster};
use crate::emit;

#[derive(Debug)]
pub struct Preferences {
    visible: bool,
    form_disabled: bool,
    server_address: adw::EntryRow,
    config: Option<Config>,
    toaster: Toaster,
}

#[derive(Debug)]
pub enum Msg {
    EditServerAddress,
    Return,
    Open,
    Hide,
    Error(&'static str),
}

#[relm4::component(pub)]
impl Component for Preferences {
    type Init = ();
    type Input = Msg;
    type Output = ();
    type CommandOutput = Option<Config>;

    view! {
        dialog = adw::Window {
            #[watch]
            set_visible: model.visible,
            set_modal: true,

            add_controller: escape_action!(Msg::Hide => sender),

            set_title: Some("Preferences"),

            grab_focus: (),

            set_default_width: 400,
            set_default_height: 500,

            adw::ToolbarView {
                set_top_bar_style: adw::ToolbarStyle::Raised,

                add_top_bar = &adw::HeaderBar { },

                #[local_ref]
                toaster -> adw::ToastOverlay {
                    adw::PreferencesGroup {
                        set_margin_horizontal: 20,
                        set_margin_vertical: 50,

                        set_title: "Preferences",

                        #[local_ref]
                        address -> adw::EntryRow {
                            set_title: "server_address",

                            #[watch]
                            set_sensitive: !model.form_disabled,

                            add_css_class: "no-edit-button",

                            set_can_focus: false,

                            #[name = "edit"]
                            add_suffix = &gtk::Button {
                                set_icon_name: icon_names::EDIT,

                                add_css_class: "flat",
                                add_css_class: "circular",

                                connect_clicked => Msg::EditServerAddress,
                            },
                        },

                        gtk::Button {
                            set_margin_all: 40,

                            add_css_class: "suggested-action",

                            set_label: "Save",
                            connect_clicked => Msg::Return,
                        }
                    }
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
            config: None,
            toaster: Toaster::default(),
            visible: false,
            form_disabled: false,
            server_address: adw::EntryRow::new(),
        };

        sender.oneshot_command(async {
            let config = *Config::get().await;
            Some(config)
        });

        let address = &model.server_address;
        let toaster = model.toaster.as_ref();

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _: &Self::Root) {
        match msg {
            Msg::Return => {
                let mut config = self.config.expect("Config should have already been loaded");

                if let Ok(server_address) = self.server_address.text().parse::<SocketAddr>() {
                    sender.oneshot_command(async move {
                        config.server_address = server_address.ip();
                        config.server_port = server_address.port();

                        // SAFETY:
                        // Called in async context
                        // config was created with `Config::get()`
                        unsafe { config.persist_server_address_change() }.await.ok()
                    });
                } else {
                    sender.input(Msg::Error("Not a valid ip address"));
                    self.server_address.add_css_class("error");
                };
            }

            Msg::EditServerAddress => {
                self.server_address.grab_focus_without_selecting();
            }

            Msg::Open => {
                self.visible = true;
            }

            Msg::Hide => {
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
        if let Some(config) = message {
            if self.config.is_none() {
                self.config = Some(config);
                self.server_address
                    .set_text(&format!("{}:{}", config.server_address, config.server_port));
            } else {
                emit!(() => sender);
                self.visible = false;
            }
        } else {
            self.server_address.add_css_class("error");
            sender.input(Msg::Error("Invalid ip address"));
        }
    }
}
