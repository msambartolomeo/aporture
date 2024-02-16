mod app;

use relm4::RelmApp;

use crate::app::App;

fn main() {
    let app = RelmApp::new("dev.msambartolomeo.aporture");
    app.run::<App>(0);
}
