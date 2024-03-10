use std::path::PathBuf;

use aporture::pairing::{AporturePairingProtocol, PairKind};
use relm4::{prelude::*, Worker};

#[derive(Debug)]
pub enum Output {
    Success,
    // TODO: Handle errors
    // Error,
}

#[derive(Debug)]
pub enum AportureInput {
    SendFile {
        passphrase: Vec<u8>,
        path: PathBuf,
    },
    RecieveFile {
        passphrase: Vec<u8>,
        destination: Option<PathBuf>,
    },
}

#[derive(Debug)]
pub struct AportureWorker;

impl Worker for AportureWorker {
    type Init = ();
    type Input = AportureInput;
    type Output = Output;

    fn init(_init: Self::Init, _sender: ComponentSender<Self>) -> Self {
        Self
    }

    fn update(&mut self, msg: AportureInput, sender: ComponentSender<Self>) {
        match msg {
            AportureInput::SendFile { passphrase, path } => {
                let pair_info = AporturePairingProtocol::new(PairKind::Sender, passphrase).pair();

                aporture::transfer::send_file(&path, &pair_info);

                sender
                    .output(Output::Success)
                    .expect("Message returned to main thread");
            }
            AportureInput::RecieveFile {
                passphrase,
                destination,
            } => {
                let pair_info = AporturePairingProtocol::new(PairKind::Reciever, passphrase).pair();

                aporture::transfer::recieve_file(destination, &pair_info);

                sender
                    .output(Output::Success)
                    .expect("Message returned to main thread");
            }
        }
    }
}
