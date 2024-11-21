use adw::prelude::*;
use gtk::gio::Cancellable;
use relm4::prelude::*;

pub struct Confirmation<'a> {
    pub heading: &'a str,
    pub confirm: Option<&'static str>,
    pub deny: Option<&'static str>,
}

impl<'a> Confirmation<'a> {
    pub const fn new(heading: &'a str) -> Self {
        Self {
            heading,
            confirm: None,
            deny: None,
        }
    }

    pub fn confirm(&mut self, confirm: &'static str) -> &mut Self {
        self.confirm = Some(confirm);
        self
    }

    pub fn deny(&mut self, deny: &'static str) -> &mut Self {
        self.deny = Some(deny);
        self
    }

    pub fn choose(&self, parent: &impl IsA<gtk::Widget>, if_yes: impl FnOnce() + 'static) {
        let confirm = self.confirm.unwrap_or("Yes");
        let deny = self.deny.unwrap_or("No");

        relm4::view! {
            dialog = adw::AlertDialog {
                set_heading: Some(&format!("You are about to {}", self.heading)),
                set_body: "This action is irreversible",

                add_response: ("yes", confirm),
                add_response: ("no", deny),

                set_response_appearance[adw::ResponseAppearance::Destructive]: "yes",

            }
        }

        dialog.choose(parent, Some(&Cancellable::default()), |r| {
            if r == "yes" {
                if_yes();
            }
        });
    }
}
