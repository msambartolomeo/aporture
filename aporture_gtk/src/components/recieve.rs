use adw::prelude::*;
use relm4::prelude::*;

#[tracker::track]
#[derive(Debug)]
pub struct RecieverPage {
    passphrase: gtk::EntryBuffer,
    passphrase_empty: bool,
}

#[derive(Debug)]
pub enum Msg {
    PassphraseChanged,
}

#[relm4::component(pub)]
impl SimpleComponent for RecieverPage {
    type Init = ();
    type Input = Msg;
    type Output = ();

    view! {
        adw::PreferencesGroup {
            set_margin_horizontal: 20,
            set_margin_vertical: 50,

            set_title: "Recieve",
            #[wrap(Some)]
            set_header_suffix = &gtk::Button {
                set_label: "Connect",
                #[track = "model.changed(RecieverPage::passphrase_empty())"]
                set_sensitive: !model.passphrase.text().is_empty(),
            },
            set_description: Some("Enter the passphrase shared by the sender"),

            gtk::Entry {
                set_tooltip_text: Some("Passphrase"),
                set_buffer: &model.passphrase,
                connect_changed[sender] => move |_| {
                    sender.input(Msg::PassphraseChanged)
                } ,
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {
            passphrase: gtk::EntryBuffer::default(),
            passphrase_empty: true,
            tracker: 0,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        self.reset();

        match msg {
            Msg::PassphraseChanged => self.set_passphrase_empty(self.passphrase.text().is_empty()),
        }
    }
}
