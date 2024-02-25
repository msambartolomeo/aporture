use std::path::PathBuf;

use adw::prelude::*;
use relm4::prelude::*;
use relm4_components::open_dialog::{
    OpenDialog, OpenDialogMsg, OpenDialogResponse, OpenDialogSettings,
};
use relm4_icons::icon_name;

#[derive(Debug)]
pub struct SenderPage {
    passphrase: gtk::EntryBuffer,
    passphrase_empty: bool,
    file_path: Option<PathBuf>,
    file_name: Option<String>,
    file_picker_dialog: Controller<OpenDialog>,
}

#[derive(Debug)]
pub enum Msg {
    PassphraseChanged,
    GeneratePassphrase,
    FilePickerOpen,
    FilePickerResponse(PathBuf),
    SendFile,
    Ignore,
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
                set_sensitive: !model.passphrase_empty && model.file_path.is_some(),

                connect_clicked[sender] => move |_| {
                    sender.input(Msg::SendFile);
                },
            },
            set_description: Some("Enter a passphrase or generate a random one"),

            gtk::Entry {
                set_margin_vertical: 10,

                set_tooltip_text: Some("Passphrase"),
                set_buffer: &model.passphrase,
                set_icon_from_icon_name: (gtk::EntryIconPosition::Secondary, Some(icon_name::UPDATE)),

                connect_changed[sender] => move |_| {
                    sender.input(Msg::PassphraseChanged);
                },

                connect_icon_press[sender] => move |_, _| {
                    sender.input(Msg::GeneratePassphrase);
                }
            },

            gtk::Button {
                set_margin_vertical: 10,

                set_label: "Choose file",
                connect_clicked => Msg::FilePickerOpen
            },

            gtk::Label {
                set_margin_vertical: 10,
                set_justify: gtk::Justification::Center,
                set_wrap: true,

                #[watch]
                set_label: &format!("Selected file:\n{}", model.file_name.as_ref().unwrap_or(&"None".to_owned())),
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let file_picker_dialog = OpenDialog::builder()
            .transient_for_native(&root)
            .launch(OpenDialogSettings::default())
            .forward(sender.input_sender(), |response| match response {
                OpenDialogResponse::Accept(path) => Msg::FilePickerResponse(path),
                OpenDialogResponse::Cancel => Msg::Ignore,
            });

        let model = Self {
            passphrase: gtk::EntryBuffer::default(),
            passphrase_empty: true,
            file_path: None,
            file_name: None,
            file_picker_dialog,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            Msg::PassphraseChanged => self.passphrase_empty = self.passphrase.length() == 0,
            Msg::GeneratePassphrase => todo!("Generate random passphrase"),
            Msg::FilePickerOpen => self.file_picker_dialog.emit(OpenDialogMsg::Open),
            Msg::FilePickerResponse(path) => {
                self.file_name = Some(
                    path.file_name()
                        .expect("Must be a file")
                        .to_string_lossy()
                        .to_string(),
                );
                self.file_path = Some(path);
            }
            Msg::SendFile => {
                log::info!("Selected passphrase is {}", self.passphrase);

                let _passphrase = self.passphrase.text().into_bytes();

                todo!("Start sending process")
            }
            Msg::Ignore => (),
        }
    }
}
