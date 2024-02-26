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
pub enum Input {
    SendFile { passphrase: Vec<u8>, path: PathBuf },
}

#[derive(Debug)]
pub struct AportureWorker;

impl Worker for AportureWorker {
    type Init = ();
    type Input = Input;
    type Output = Output;

    fn init(_init: Self::Init, _sender: ComponentSender<Self>) -> Self {
        Self
    }

    fn update(&mut self, msg: Input, sender: ComponentSender<Self>) {
        match msg {
            Input::SendFile { passphrase, path } => {
                let pair_info = AporturePairingProtocol::new(PairKind::Sender, passphrase).pair();

                aporture::transfer::send_file(path, pair_info);

                sender
                    .output(Output::Success)
                    .expect("Message returned to main thread");
            }
        }
    }
}
