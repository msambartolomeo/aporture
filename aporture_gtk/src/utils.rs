#[macro_export]
macro_rules! emit {
    ($msg:expr => $sender:expr) => {
        $sender
            .output($msg)
            .expect("Parent controller not detached")
    };
}
