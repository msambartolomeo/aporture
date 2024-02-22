use adw::prelude::*;
use relm4::prelude::*;

#[derive(Debug)]
pub struct SenderPage {
    passphrase: gtk::EntryBuffer,
    passphrase_empty: bool,
}

#[derive(Debug)]
pub enum Msg {
    PassphraseChanged,
    GeneratePassphrase,
    SendFile,
}

#[relm4::component(pub)]
impl SimpleComponent for SenderPage {
    type Init = ();
    type Input = Msg;
    type Output = ();

    view! {
        adw::PreferencesGroup {
            set_margin_horizontal: 20,
            set_margin_vertical: 50,

            set_title: "Send",
            #[wrap(Some)]
            set_header_suffix = &gtk::Button {
                set_label: "Connect",
                #[watch]
                set_sensitive: !model.passphrase_empty,

                connect_clicked[sender] => move |_| {
                    sender.input(Msg::SendFile);
                },
            },
            set_description: Some("Enter a passphrase or generate a random one"),

            gtk::Entry {
                set_tooltip_text: Some("Passphrase"),
                set_buffer: &model.passphrase,

                set_icon_from_icon_name: (gtk::EntryIconPosition::Secondary, Some("update")),

                connect_changed[sender] => move |_| {
                    sender.input(Msg::PassphraseChanged);
                },

                connect_icon_press[sender] => move |_, _| {
                    sender.input(Msg::GeneratePassphrase);
                }
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
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            Msg::PassphraseChanged => self.passphrase_empty = self.passphrase.length() == 0,
            Msg::GeneratePassphrase => todo!("Generate random passphrase"),
            Msg::SendFile => {
                log::info!("Selected passphrase is {}", self.passphrase);

                let _passphrase = self.passphrase.text().into_bytes();

                todo!("Start sending process")
            }
        }
    }
}
