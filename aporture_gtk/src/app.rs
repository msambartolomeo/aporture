use adw::prelude::*;
use relm4::prelude::*;

#[derive(Debug)]
pub struct App;

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

                #[name = "navigation"]
                add_bottom_bar = &adw::ViewSwitcherBar {
                    set_reveal: true,
                },

                #[name = "stack"]
                adw::ViewStack {
                    add_titled_with_icon[None, "Send", "send"] = &adw::StatusPage {
                        set_title: "Send",
                    },


                    add_titled_with_icon[None, "Recieve", "inbox"] = &gtk::Label {
                        set_label: "Recieve",
                    }
                },
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {};

        let widgets = view_output!();

        widgets.navigation.set_stack(Some(&widgets.stack));

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
