#![allow(unused)]
#![allow(unused_variables)]
// TODO: DEPRECATED, REMOVE FILE
use std::path::PathBuf;

use aporture::pairing::{AporturePairingProtocol, Receiver, Sender};

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
    ReceiveFile {
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
        todo!();

        // match msg {
        //     AportureInput::SendFile { passphrase, path } => {
        //         let mut pair_info = AporturePairingProtocol::<Sender>::new(passphrase)
        //             .pair()
        //             .unwrap();

        //         aporture::transfer::send_file(&path, &mut pair_info).await;

        //         sender
        //             .output(Output::Success)
        //             .expect("Message returned to main thread");
        //     }
        //     AportureInput::ReceiveFile {
        //         passphrase,
        //         destination,
        //     } => {
        //         let mut pair_info = AporturePairingProtocol::<Receiver>::new(passphrase)
        //             .pair()
        //             .unwrap();

        //         aporture::transfer::receive_file(destination, &mut pair_info).await;

        //         sender
        //             .output(Output::Success)
        //             .expect("Message returned to main thread");
        //     }
        // }
    }
}
