use std::sync::Arc;

use adw::prelude::*;
use aporture::fs::contacts::Contacts;
use relm4::gtk::glib::GString;
use relm4::prelude::*;
use relm4_icons::icon_names;
use tokio::sync::Mutex;

use crate::components::contacts::{self, ContactPage};
use crate::components::dialog::contacts::{Holder as ContactHolder, Msg as ContactMsg};
use crate::components::receive::{self, ReceiverPage};
use crate::components::send::{self, SenderPage};

#[derive(Debug)]
pub struct App {
    stack: adw::ViewStack,
    toaster: adw::ToastOverlay,
    receive_page: Controller<ReceiverPage>,
    sender_page: Controller<SenderPage>,
    contacts_page: Controller<ContactPage>,
    contacts_holder: Controller<ContactHolder>,
    current_page: GString,
    contacts: Option<Arc<Mutex<Contacts>>>,
}

const CONTACTS_PAGE_NAME: &str = "Contacts";
const SENDER_PAGE_NAME: &str = "Send";
const RECEIVER_PAGE_NAME: &str = "Receive";

#[derive(Debug)]
pub enum Msg {
    Contacts(Option<Arc<Mutex<Contacts>>>),
    ContactsRequest,
    PageSwitch,
    Toast(String),
}

#[derive(Debug)]
pub enum Request {
    Contacts,
    Toast(String),
}

impl From<Request> for Msg {
    fn from(value: Request) -> Self {
        match value {
            Request::Contacts => Self::ContactsRequest,
            Request::Toast(message) => Self::Toast(message),
        }
    }
}

#[relm4::component(pub)]
impl SimpleComponent for App {
    type Init = ();
    type Input = Msg;
    type Output = ();

    view! {
        #[root]
        adw::ApplicationWindow {
            set_title: Some("Aporture"),
            set_default_width: 550,
            set_default_height: 650,

            adw::ToolbarView {
                set_top_bar_style: adw::ToolbarStyle::Raised,
                set_bottom_bar_style: adw::ToolbarStyle::Raised,

                add_top_bar = &adw::HeaderBar {},

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

        let toaster = adw::ToastOverlay::default();

        let model = Self {
            stack: adw::ViewStack::default(),
            receive_page,
            sender_page,
            contacts_page,
            contacts_holder,
            toaster,
            current_page: SENDER_PAGE_NAME.into(),
            contacts: None,
        };

        let stack = &model.stack;
        let toaster = &model.toaster;

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            Msg::Contacts(contacts) => {
                if let Some(contacts) = contacts {
                    self.contacts = Some(contacts);
                }

                if self.contacts.is_none() {
                    self.stack.set_visible_child_name(&self.current_page);
                } else if let Some(page) = self.stack.visible_child_name() {
                    self.current_page = page;
                }

                self.sender_page
                    .emit(send::Msg::ContactsReady(self.contacts.clone()));

                self.receive_page
                    .emit(receive::Msg::ContactsReady(self.contacts.clone()));

                self.contacts_page
                    .emit(contacts::Msg::ContactsReady(self.contacts.clone()));
            }

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

            Msg::Toast(message) => {
                relm4::view! {
                    title = gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,

                        gtk::Label {
                            set_text: &message
                        }
                    }
                }

                let toast = adw::Toast::builder()
                    .timeout(3)
                    // .title(&message)
                    .custom_title(&title)
                    .build();

                self.toaster.add_toast(toast);
            }
        }
    }
}
