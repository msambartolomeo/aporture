use std::path::PathBuf;

use adw::prelude::*;
use aporture::passphrase;
use relm4::prelude::*;
use relm4_components::open_dialog::{
    OpenDialog, OpenDialogMsg, OpenDialogResponse, OpenDialogSettings,
};
use relm4_icons::icon_names;

use crate::components::dialog::{AportureInput, AportureTransfer, Purpose};

const PASSPHRASE_WORD_COUNT: usize = 3;

#[derive(Debug)]
pub struct SenderPage {
    passphrase_entry: adw::EntryRow,
    file_path: Option<PathBuf>,
    file_name: Option<String>,
    file_picker_dialog: Controller<OpenDialog>,
    aporture_dialog: Controller<AportureTransfer>,
    form_disabled: bool,
}

#[derive(Debug)]
pub enum Msg {
    GeneratePassphrase,
    FilePickerOpen,
    FilePickerResponse(PathBuf),
    SendFile,
    SendFileFinished,
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

            set_width_request: 250,

            set_title: "Send",
            set_description: Some("Enter a passphrase or generate a random one"),
            #[wrap(Some)]
            set_header_suffix = &gtk::Button {
                set_label: "Connect",
                #[watch]
                set_sensitive: !model.form_disabled && passphrase_entry.text_length() != 0 && model.file_path.is_some(),
                connect_clicked[sender] => move |_| {
                    sender.input(Msg::SendFile);
                },
            },

            #[local_ref]
            passphrase_entry -> adw::EntryRow {
                set_title: "Passphrase",
                set_text: &passphrase::generate(PASSPHRASE_WORD_COUNT),
                #[watch]
                set_sensitive: !model.form_disabled,

                add_suffix = &gtk::Button {
                    set_icon_name: icon_names::UPDATE,

                    connect_clicked[sender] => move |_| {
                        sender.input(Msg::GeneratePassphrase);
                    }
                }
            },

            gtk::Button {
                set_margin_vertical: 10,

                set_label: "Choose file",
                #[watch]
                set_sensitive: !model.form_disabled,

                connect_clicked => Msg::FilePickerOpen
            },

            gtk::Label {
                set_margin_vertical: 10,
                set_justify: gtk::Justification::Center,
                set_wrap: true,

                #[watch]
                set_label: &format!("Selected file:\n{}", model.file_name.as_ref().unwrap_or(&"None".to_owned())),
                #[watch]
                set_sensitive: !model.form_disabled,
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

        let aporture_dialog = AportureTransfer::builder()
            .transient_for(&root)
            .launch(Purpose::Send)
            .forward(sender.input_sender(), |_| Msg::SendFileFinished); // TODO: Handle Errors

        let model = Self {
            passphrase_entry: adw::EntryRow::default(),
            file_path: None,
            file_name: None,
            file_picker_dialog,
            aporture_dialog,
            form_disabled: false,
        };

        let passphrase_entry = &model.passphrase_entry;

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            Msg::GeneratePassphrase => self
                .passphrase_entry
                .set_text(&passphrase::generate(PASSPHRASE_WORD_COUNT)),

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
                self.form_disabled = true;

                let passphrase = self.passphrase_entry.text();

                log::info!("Selected passphrase is {}", passphrase);

                let passphrase = passphrase.into_bytes();

                log::info!("Starting sender worker");
                self.aporture_dialog.emit(AportureInput::SendFile {
                    passphrase,
                    path: self.file_path.clone().expect("Button disabled if None"),
                });
            }

            Msg::SendFileFinished => {
                log::info!("Finished sender worker");

                self.form_disabled = false;
            }

            Msg::Ignore => (),
        }
    }
}
