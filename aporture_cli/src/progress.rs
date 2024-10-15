use std::fmt::Write;

use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;

pub fn init_progress_bar(mut channel: Receiver<usize>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let Some(total) = channel.recv().await else {
            return;
        };

        let progress = ProgressBar::new(total as u64);

        progress.set_style(style());

        while let Some(n) = channel.recv().await {
            progress.inc(n as u64);
        }

        progress.finish();
    })
}

pub fn style() -> ProgressStyle {
    ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
        .expect("Template is valid as it does not change")
        .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).expect("ETA does not fail to write"))
        .progress_chars("#>-")
}
