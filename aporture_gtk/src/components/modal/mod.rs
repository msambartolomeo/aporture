pub mod aporture;
pub mod contacts;
pub mod preferences;

#[macro_use]
mod utils {
    #[macro_export]
    macro_rules! escape_action {
        ($action:expr => $sender:expr) => {{
            let escape_closes = gtk::EventControllerKey::default();

            let s = $sender.clone();
            escape_closes.connect_key_pressed(move |_, key, _, _| {
                if matches!(key, gtk::gdk::Key::Escape) {
                    s.input($action);
                }

                gtk::glib::Propagation::Proceed
            });

            escape_closes
        }};
    }

    pub use escape_action;
}
