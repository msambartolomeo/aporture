use std::sync::Arc;

use adw::prelude::*;
use aporture::fs::contacts::Contacts;
use relm4::gtk::glib::GString;
use relm4::prelude::*;
use relm4_icons::icon_names;
use tokio::sync::RwLock;

// use crate::components::contacts::{self, ContactsPage};
use crate::components::dialog::contacts::{Holder as ContactHolder, Msg as ContactMsg};
use crate::components::receive::{self, ReceiverPage};
use crate::components::send::{self, SenderPage};

#[derive(Debug)]
pub struct App {
    stack: adw::ViewStack,
    receive_page: Controller<ReceiverPage>,
    sender_page: Controller<SenderPage>,
    // contacts_page: Controller<ContactsPage>,
    contacts_holder: Controller<ContactHolder>,
    current_page: GString,
    contacts: Option<Arc<RwLock<Contacts>>>,
}

const CONTACTS_PAGE_NAME: &str = "Contacts";
const SENDER_PAGE_NAME: &str = "Send";
const RECEIVER_PAGE_NAME: &str = "Receive";

#[derive(Debug)]
pub enum Msg {
    Contacts(Option<Arc<RwLock<Contacts>>>),
    ContactsRequest,
    PageSwitch,
}

#[derive(Debug)]
pub enum Request {
    Contacts,
}

#[relm4::component(pub)]
impl SimpleComponent for App {
    type Init = ();
    type Input = Msg;
    type Output = ();

    view! {
        #[root]
        adw::Window {
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
                stack -> adw::ViewStack {
                    set_margin_horizontal: 40,

                    connect_visible_child_name_notify => Msg::PageSwitch,

                    add_titled_with_icon[Some(SENDER_PAGE_NAME), SENDER_PAGE_NAME, icon_names::SEND] = model.sender_page.widget(),

                    add_titled_with_icon[Some(RECEIVER_PAGE_NAME), RECEIVER_PAGE_NAME, icon_names::INBOX] = model.receive_page.widget(),

                    // add_titled_with_icon[Some(CONTACTS_PAGE_NAME), CONTACTS_PAGE_NAME, icon_names::ADDRESS_BOOK] = model.receive_page.widget(),
                },
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
            .forward(sender.input_sender(), |r| match r {
                Request::Contacts => Msg::ContactsRequest,
            });

        let sender_page = SenderPage::builder()
            .launch(())
            .forward(sender.input_sender(), |r| match r {
                Request::Contacts => Msg::ContactsRequest,
            });

        // let contacts_page = SenderPage::builder()
        //     .launch(())
        //     .forward(sender.input_sender(), |r| match r {
        //         Request::Contacts => Msg::ContactsRequest,
        //     });

        let contacts_holder = ContactHolder::builder()
            .transient_for(&root)
            .launch(())
            .forward(sender.input_sender(), Msg::Contacts);

        let model = Self {
            stack: adw::ViewStack::default(),
            receive_page,
            sender_page,
            // contacts_page,
            contacts_holder,
            current_page: SENDER_PAGE_NAME.into(),
            contacts: None,
        };

        let stack = &model.stack;

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            Msg::Contacts(contacts) => {
                self.contacts = contacts;

                if self.contacts.is_none() {
                    self.stack.set_visible_child_name(&self.current_page);
                } else {
                    if let Some(page) = self.stack.visible_child_name() {
                        self.current_page = page;
                    }
                }

                self.sender_page
                    .emit(send::Msg::ContactsReady(self.contacts.clone()));

                self.receive_page
                    .emit(receive::Msg::ContactsReady(self.contacts.clone()));

                // self.contacts_page
                //     .emit(contacts::Msg::ContactsReady(self.contacts.clone()));
            }

            Msg::ContactsRequest => self.contacts_holder.emit(ContactMsg::Get),

            Msg::PageSwitch => {
                if let Some(page) = self.stack.visible_child_name() {
                    if page == self.current_page || self.contacts.is_some() {
                        return;
                    }

                    // if page == CONTACTS_PAGE_NAME {
                    if page == RECEIVER_PAGE_NAME {
                        sender.input(Msg::ContactsRequest);
                    } else {
                        self.current_page = page;
                    }
                }
            }
        }
    }
}
