use adw::prelude::*;
use relm4::prelude::*;

use crate::components::recieve::RecieverPage;
use crate::components::send::SenderPage;

#[derive(Debug)]
pub struct App {
    recieve_page: Controller<RecieverPage>,
    // send_page: Controller<Sender>,
}

#[derive(Debug)]
pub enum Msg {}

#[relm4::component(pub)]
impl SimpleComponent for App {
    type Init = ();
    type Input = Msg;
    type Output = ();

    view! {
        #[root]
        adw::Window {
            set_title: Some("Aporture"),
            set_default_width: 300,

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
                    add_titled_with_icon[None, "Send", "send"] = &adw::StatusPage {
                        set_title: "Send",
                    },

                    add_titled_with_icon[None, "Recieve", "inbox"] = model.recieve_page.widget(),
                },
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let recieve_page: Controller<RecieverPage> = RecieverPage::builder().launch(()).detach();

        let model = Self { recieve_page };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, _msg: Self::Input, _sender: ComponentSender<Self>) {
        // match msg {
        //     Msg::Increment => {
        //         self.counter = self.counter.wrapping_add(1);
        //     }
        //     Msg::Decrement => {
        //         self.counter = self.counter.wrapping_sub(1);
        //     }
        // }
    }
}
