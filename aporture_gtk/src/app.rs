use std::sync::Arc;

use adw::prelude::*;
use aporture::fs::contacts::Contacts;
use relm4::actions::{RelmAction, RelmActionGroup};
use relm4::gtk::glib::{clone, GString};
use relm4::prelude::*;
use relm4_icons::icon_names;
use tokio::sync::Mutex;

use crate::components::modal::contacts::{
    Holder as ContactHolder, Msg as ContactMsg, Output as ContactOutput,
};
use crate::components::modal::preferences::{self, Preferences};
use crate::components::toaster::{Severity, Toaster};
use crate::pages::{contacts, receive, send, ContactPage, ReceiverPage, SenderPage};

#[derive(Debug)]
pub struct App {
    stack: adw::ViewStack,
    toaster: Toaster,
    receive_page: Controller<ReceiverPage>,
    sender_page: Controller<SenderPage>,
    contacts_page: Controller<ContactPage>,
    contacts_holder: Controller<ContactHolder>,
    preferences: Controller<Preferences>,
    current_page: GString,
    contacts: Option<Arc<Mutex<Contacts>>>,
}

const CONTACTS_PAGE_NAME: &str = "Contacts";
const SENDER_PAGE_NAME: &str = "Send";
const RECEIVER_PAGE_NAME: &str = "Receive";

#[derive(Debug)]
pub enum Msg {
    Contacts(ContactOutput),
    ContactsRequest,
    PageSwitch,
    Toast(String, Severity),
    ToastS(&'static str, Severity),
    About,
    Preferences,
}

#[derive(Debug)]
pub enum Request {
    Contacts,
    Toast(String, Severity),
    ToastS(&'static str, Severity),
}

impl From<Request> for Msg {
    fn from(value: Request) -> Self {
        match value {
            Request::Contacts => Self::ContactsRequest,
            Request::Toast(message, severity) => Self::Toast(message, severity),
            Request::ToastS(message, severity) => Self::ToastS(message, severity),
        }
    }
}

#[relm4::component(pub)]
impl Component for App {
    type Init = ();
    type Input = Msg;
    type Output = ();
    type CommandOutput = ();

    menu! {
        menu: {
            "Preferences" => PreferencesAction,
            "About Aporture" => AboutAction,
        }
    }

    view! {
        #[root]
        adw::ApplicationWindow {
            set_title: Some("Aporture"),
            set_default_width: 550,
            set_default_height: 650,

            adw::ToolbarView {
                set_top_bar_style: adw::ToolbarStyle::Raised,
                set_bottom_bar_style: adw::ToolbarStyle::Raised,

                add_top_bar = &adw::HeaderBar {
                    pack_end = &gtk::MenuButton {
                        set_icon_name: icon_names::MENU,
                        set_menu_model: Some(&menu),
                    }
                },

                add_bottom_bar = &adw::HeaderBar {
                    set_show_end_title_buttons: false,

                    #[wrap(Some)]
                    #[name = "navigation"]
                    set_title_widget = &adw::ViewSwitcher {
                        set_policy: adw::ViewSwitcherPolicy::Wide,
                        set_stack: Some(&model.stack),
                    },
                },

                #[local_ref]
                toaster -> adw::ToastOverlay {
                    #[local_ref]
                    stack -> adw::ViewStack {
                        set_margin_horizontal: 40,

                        connect_visible_child_name_notify => Msg::PageSwitch,

                        add_titled_with_icon[Some(SENDER_PAGE_NAME), SENDER_PAGE_NAME, icon_names::SEND] = model.sender_page.widget(),

                        add_titled_with_icon[Some(RECEIVER_PAGE_NAME), RECEIVER_PAGE_NAME, icon_names::INBOX] = model.receive_page.widget(),

                        add_titled_with_icon[Some(CONTACTS_PAGE_NAME), CONTACTS_PAGE_NAME, icon_names::ADDRESS_BOOK] = model.contacts_page.widget(),
                    },
                }
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let receive_page = ReceiverPage::builder()
            .launch(())
            .forward(sender.input_sender(), Msg::from);

        let sender_page = SenderPage::builder()
            .launch(())
            .forward(sender.input_sender(), Msg::from);

        let contacts_page = ContactPage::builder()
            .launch(())
            .forward(sender.input_sender(), Msg::from);

        let contacts_holder = ContactHolder::builder()
            .transient_for(&root)
            .launch(())
            .forward(sender.input_sender(), Msg::Contacts);

        let preferences = Preferences::builder()
            .transient_for(&root)
            .launch(())
            .forward(sender.input_sender(), |()| {
                Msg::ToastS("Preferences updated successfully", Severity::Success)
            });

        let model = Self {
            stack: adw::ViewStack::default(),
            toaster: Toaster::default(),
            receive_page,
            sender_page,
            contacts_page,
            contacts_holder,
            preferences,
            current_page: SENDER_PAGE_NAME.into(),
            contacts: None,
        };

        let stack = &model.stack;
        let toaster = model.toaster.as_ref();

        let widgets = view_output!();

        Self::register_actions(&sender);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>, root: &Self::Root) {
        match message {
            Msg::Contacts(output) => match output {
                ContactOutput::Cancel => {
                    if self.contacts.is_none() {
                        self.stack.set_visible_child_name(&self.current_page);
                    }
                }
                ContactOutput::Contacts(contacts) => {
                    self.contacts = Some(contacts);

                    self.sender_page
                        .emit(send::Msg::ContactsReady(self.contacts.clone()));

                    self.receive_page
                        .emit(receive::Msg::ContactsReady(self.contacts.clone()));

                    self.contacts_page
                        .emit(contacts::Msg::ContactsReady(self.contacts.clone()));
                }
                ContactOutput::Error(message) => {
                    sender.input(Msg::ToastS(message, Severity::Error));
                }
            },

            Msg::ContactsRequest => self.contacts_holder.emit(ContactMsg::Get),

            Msg::PageSwitch => {
                if let Some(page) = self.stack.visible_child_name() {
                    if page == self.current_page || self.contacts.is_some() {
                        self.current_page = page;
                        return;
                    }

                    if page == CONTACTS_PAGE_NAME {
                        sender.input(Msg::ContactsRequest);
                    } else {
                        self.current_page = page;
                    }
                }
            }

            Msg::Toast(msg, severity) => self.toaster.add_toast(&msg, severity),

            Msg::ToastS(msg, severity) => self.toaster.add_toast(msg, severity),

            Msg::About => {
                let about_dialog = adw::AboutDialog::builder()
                    // .application_icon("dev.msambartolomeo.aporture")
                    .website("https://github.com/msambartolomeo/aporture")
                    .developer_name("Mauro Sambartolomeo")
                    .application_name("Aporture")
                    .build();

                about_dialog.present(Some(root));
            }

            Msg::Preferences => {
                self.preferences.emit(preferences::Msg::Open);
            }
        }
    }
}

relm4::new_action_group!(AppActions, "app");
relm4::new_stateless_action!(PreferencesAction, AppActions, "preferences");
relm4::new_stateless_action!(AboutAction, AppActions, "about");

impl App {
    fn register_actions(sender: &ComponentSender<Self>) {
        let mut actions = RelmActionGroup::<AppActions>::new();

        actions.add_action({
            RelmAction::<AboutAction>::new_stateless(clone!(
                #[strong]
                sender,
                move |_| {
                    sender.input(Msg::About);
                }
            ))
        });

        actions.add_action({
            RelmAction::<PreferencesAction>::new_stateless(clone!(
                #[strong]
                sender,
                move |_| {
                    sender.input(Msg::Preferences);
                }
            ))
        });

        actions.register_for_main_application();
    }
}
