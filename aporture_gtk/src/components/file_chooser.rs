use adw::prelude::*;
use gtk::gio::Cancellable;
use relm4::{prelude::*, Sender};
use relm4_components::open_dialog::OpenDialogMsg;

pub fn choose(
    parent: &impl IsA<gtk::Widget>,
    file_sender: Sender<OpenDialogMsg>,
    folder_sender: Sender<OpenDialogMsg>,
) {
    relm4::view! {
        dialog = adw::AlertDialog {
            set_heading: Some("Select type of transfer"),
            set_body: "Do you want to send a file or a folder?",

            add_response: ("file", "File"),
            add_response: ("folder", "Folder"),

            set_response_appearance[adw::ResponseAppearance::Suggested]: "file",
            set_response_appearance[adw::ResponseAppearance::Suggested]: "folder"
        }
    }

    dialog.choose(parent, Some(&Cancellable::default()), move |r| {
        if r == "file" {
            file_sender.emit(OpenDialogMsg::Open);
        } else if r == "folder" {
            folder_sender.emit(OpenDialogMsg::Open);
        }
    });
}
