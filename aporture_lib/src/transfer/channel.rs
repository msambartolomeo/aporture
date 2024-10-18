pub type Channel = tokio::sync::mpsc::Sender<Message>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    Compression,
    ProgressSize(usize),
    Progress(usize),
    Uncompressing,
    Finished,
}

pub async fn send(channel: &Option<Channel>, message: Message) {
    if let Some(ref channel) = channel {
        let _ = channel.send(message).await;
    }
}
