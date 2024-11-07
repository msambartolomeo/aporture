use adw::prelude::*;
use gtk::gio::Cancellable;
use relm4::prelude::*;

pub struct Confirmation<'a> {
    pub heading: &'a str,
}

impl<'a> Confirmation<'a> {
    pub fn new(heading: &'a str) -> Self {
        Self { heading }
    }

    pub fn choose(&self, parent: &impl IsA<gtk::Widget>, if_yes: impl FnOnce() + 'static) {
        relm4::view! {
            dialog = adw::AlertDialog {
                set_heading: Some(&format!("You are about to {}?", self.heading)),
                set_body: "This action is irreversible",

                add_response: ("yes", "Yes"),
                add_response: ("no", "No"),

                set_response_appearance[adw::ResponseAppearance::Destructive]: "yes"

            }
        }

        dialog.choose(parent, Some(&Cancellable::default()), |r| {
            if r == "yes" {
                if_yes()
            }
        })
    }
}
