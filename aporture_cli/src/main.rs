mod args;
use args::{AportureArgs, Commands};

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let args = AportureArgs::parse();

    dbg!(&args);

    match args.command {
        Commands::Send {
            file_name,
            method,
            save,
        } => todo!(),
        Commands::Recieve { method, save } => todo!(),
        Commands::Contacts => todo!(),
        Commands::Pair { command: _ } => todo!(),
    }
}
