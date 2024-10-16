use std::fmt::Write;

use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;

use aporture::transfer::ChannelMessage;

pub fn init_progress_bar(mut channel: Receiver<ChannelMessage>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut progress = None;

        while let Some(message) = channel.recv().await {
            match message {
                ChannelMessage::Compression => {
                    println!("The folder to send had too many files!");
                    println!("Please be patient, it will be compressed before the transfer...");
                }
                ChannelMessage::ProgressSize(total) => {
                    let p = ProgressBar::new(total as u64);
                    p.set_style(style());
                    progress = Some(p);
                }
                ChannelMessage::Progress(n) => {
                    if let Some(ref p) = progress {
                        p.inc(n as u64);
                    }
                }
                ChannelMessage::Uncompressing => {
                    println!("Waiting for transfered file to be uncompressed...");
                }
                ChannelMessage::Finished => {
                    if let Some(p) = progress.take() {
                        p.finish();
                    }
                }
            }
        }
    })
}

pub fn style() -> ProgressStyle {
    ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
        .expect("Template is valid as it does not change")
        .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).expect("ETA does not fail to write"))
        .progress_chars("#>-")
}
