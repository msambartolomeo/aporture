use relm4::ComponentSender;
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;

use aporture::transfer::ChannelMessage;

use super::{Msg, Peer, State};

pub fn handle_progress(
    mut channel: Receiver<ChannelMessage>,
    sender: ComponentSender<Peer>,
) -> JoinHandle<()> {
    relm4::spawn(async move {
        while let Some(message) = channel.recv().await {
            let input = match message {
                ChannelMessage::Compression => Msg::UpdateState(State::Compress),
                ChannelMessage::ProgressSize(total) => Msg::UpdateState(State::Sending(total)),
                ChannelMessage::Uncompressing => Msg::UpdateState(State::Uncompress),
                ChannelMessage::Finished => Msg::UpdateState(State::Final),
                ChannelMessage::Progress(n) => Msg::Progress(n),
            };

            sender.input(input);
        }
    })
}

pub fn handle_pulse(sender: ComponentSender<Peer>) -> JoinHandle<()> {
    relm4::spawn(async move {
        loop {
            sender.input(Msg::Pulse);
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
}
