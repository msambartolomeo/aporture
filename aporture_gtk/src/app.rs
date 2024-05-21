use std::sync::{Arc, RwLock};

use adw::prelude::*;
use aporture::fs::contacts::Contacts;
use relm4::prelude::*;
use relm4_icons::icon_names;

use crate::components::dialog::contacts;
use crate::components::receive::{self, ReceiverPage};
use crate::components::send::{self, SenderPage};

#[derive(Debug)]
pub struct App {
    receive_page: Controller<ReceiverPage>,
    sender_page: Controller<SenderPage>,
    contacts_holder: Controller<contacts::Holder>,
}

#[derive(Debug)]
pub enum Msg {
    Contacts(Option<Arc<RwLock<Contacts>>>),
    ContactsRequest,
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
                        set_stack: Some(&stack),
                    },
                },

                #[name = "stack"]
                adw::ViewStack {
                    set_margin_horizontal: 40,

                    add_titled_with_icon[None, "Send", icon_names::SEND] = model.sender_page.widget(),

                    add_titled_with_icon[None, "Receive", icon_names::INBOX] = model.receive_page.widget(),
                },
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let receive_page = ReceiverPage::builder().launch(()).detach();
        let sender_page = SenderPage::builder()
            .launch(())
            .forward(sender.input_sender(), |r| match r {
                Request::Contacts => Msg::ContactsRequest,
            });

        let contacts_holder = contacts::Holder::builder()
            .transient_for(&root)
            .launch(())
            .forward(sender.input_sender(), |contacts| Msg::Contacts(contacts));

        let model = Self {
            receive_page,
            sender_page,
            contacts_holder,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            Msg::Contacts(contacts) => {
                self.sender_page.emit(send::Msg::ContactsReady(contacts));

                // self.receive_page
                //     .emit(receive::Msg::ContactsReady(contacts.clone()));
            }
            Msg::ContactsRequest => self.contacts_holder.emit(contacts::Msg::Get),
        }
    }
}
