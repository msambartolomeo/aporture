use std::path::PathBuf;
use std::sync::Arc;

use adw::prelude::*;
use relm4::prelude::*;
use tokio::sync::RwLock;

use crate::components::error::Aporture;
use aporture::fs::contacts::Contacts;
use aporture::pairing::{AporturePairingProtocol, Receiver, Sender};

#[derive(Debug)]
pub struct AportureTransfer {
    visible: bool,
    label: &'static str,
}

#[derive(Debug)]
pub enum AportureInput {
    SendFile {
        passphrase: PassphraseMethod,
        path: PathBuf,
        save: Option<(String, Arc<RwLock<Contacts>>)>,
    },
    ReceiveFile {
        passphrase: PassphraseMethod,
        destination: Option<PathBuf>,
        save: Option<(String, Arc<RwLock<Contacts>>)>,
    },
}

#[derive(Debug)]
pub enum PassphraseMethod {
    Direct(Vec<u8>),
    Contact(String, Arc<RwLock<Contacts>>),
}

#[relm4::component(pub)]
impl Component for AportureTransfer {
    type Init = ();
    type Input = AportureInput;
    type Output = Result<(), Aporture>;
    type CommandOutput = Result<(), Aporture>;

    view! {
        dialog = gtk::Window {
            #[watch]
            set_visible: model.visible,
            set_modal: true,

            #[wrap(Some)]
            set_child = &gtk::Label {
                set_width_request: 250,
                set_height_request: 400,
                set_halign: gtk::Align::Center,
                set_valign: gtk::Align::Center,
                #[watch]
                set_label: model.label,
            },

            // connect_close_request[sender] => move |_| {
            //     sender.input(DialogMsg::Hide);
            //     glib::Propagation::Stop
            // }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {
            visible: false,
            label: "",
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _: &Self::Root) {
        self.visible = true;
        // TODO: Show errors in screen

        match msg {
            AportureInput::SendFile {
                passphrase,
                path,
                save,
            } => {
                sender.oneshot_command(async move {
                    let passphrase = match passphrase {
                        PassphraseMethod::Direct(p) => p,
                        PassphraseMethod::Contact(name, contacts) => {
                            contacts.read().await.get(&name).unwrap().to_vec()
                        }
                    };

                    // self.label = "Waiting for your peer...";

                    let mut pair_info =
                        AporturePairingProtocol::<Sender>::new(passphrase, save.is_some())
                            .pair()
                            .await?;

                    log::info!("Pairing successful!");

                    // self.label = "Pairing successful!!\nTransferring file to peer";

                    aporture::transfer::send_file(&path, &mut pair_info).await?;

                    let save_confirmation = pair_info.save_contact;

                    let key = pair_info.finalize().await;

                    if let Some((name, contacts)) = save {
                        if save_confirmation {
                            let mut contacts = contacts.write().await;
                            contacts.add(name, key);
                            contacts.save().await.unwrap();
                        } else {
                            // self.label = "Warning: Not saving contact because peer refused";
                        }
                    }

                    Ok(())
                });
            }
            AportureInput::ReceiveFile {
                passphrase,
                destination,
                save,
            } => {
                sender.oneshot_command(async {
                    let passphrase = match passphrase {
                        PassphraseMethod::Direct(p) => p,
                        PassphraseMethod::Contact(name, contacts) => {
                            contacts.read().await.get(&name).unwrap().to_vec()
                        }
                    };

                    let mut pair_info = AporturePairingProtocol::<Receiver>::new(passphrase, false)
                        .pair()
                        .await?;

                    aporture::transfer::receive_file(destination, &mut pair_info).await?;

                    let save_confirmation = pair_info.save_contact;

                    let key = pair_info.finalize().await;

                    if let Some((name, contacts)) = save {
                        if save_confirmation {
                            let mut contacts = contacts.write().await;
                            contacts.add(name, key);
                            contacts.save().await.unwrap();
                        } else {
                            // self.label = "Warning: Not saving contact because peer refused";
                        }
                    }

                    Ok(())
                });
            }
        }
    }

    fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        sender: ComponentSender<Self>,
        _: &Self::Root,
    ) {
        sender
            .output(message)
            .expect("Message returned to the main thread");

        self.visible = false;
    }
}
