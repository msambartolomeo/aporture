use indicatif::ProgressBar;
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;

pub fn init_progress_bar(mut channel: Receiver<usize>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let Some(total) = channel.recv().await else {
            return;
        };

        let progress = ProgressBar::new(total as u64);

        while let Some(n) = channel.recv().await {
            progress.inc(n as u64);
        }

        progress.finish();
    })
}
