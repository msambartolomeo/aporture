mod app;

use relm4::RelmApp;

use crate::app::App;

fn init_logger() {
    use std::io::Write;

    env_logger::Builder::from_default_env()
        .format(|buf, record| {
            let color = buf.default_level_style(record.level());

            writeln!(
                buf,
                "{}:{} {} {color}{}{color:#} - {}",
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                buf.timestamp(),
                record.level(),
                record.args()
            )
        })
        .init();
}

fn main() {
    init_logger();

    let app = RelmApp::new("dev.msambartolomeo.aporture");

    log::info!("Application starting");

    app.run::<App>(0);

    log::info!("Aplication Closing");
}
