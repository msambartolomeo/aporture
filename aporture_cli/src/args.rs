use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "Aporture", author, version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Send a file
    Send {
        path: PathBuf,

        #[command(flatten)]
        method: SendMethod,

        #[arg(short, long, value_names(["NAME"]))]
        save: Option<String>,
    },
    /// Recieve a file
    Recieve {
        #[arg(short, long, value_names(["PATH"]))]
        destination: Option<PathBuf>,

        #[command(flatten)]
        method: RecieveMethod,

        #[arg(short, long, value_names(["NAME"]))]
        save: Option<String>,
    },
    /// List registered contacts
    Contacts,
    /// Pair a new contact
    Pair {
        #[command(subcommand)]
        command: PairCommand,
    },
}

#[derive(Debug, Args)]
#[group(multiple = false)]
pub struct SendMethod {
    #[arg(short, long)]
    pub passphrase: Option<String>,

    #[arg(short, long, value_names(["NAME"]))]
    pub contact: Option<String>,
}

#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
pub struct RecieveMethod {
    pub passphrase: Option<String>,

    #[arg(short, long, value_names(["NAME"]))]
    pub contact: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum PairCommand {
    Start {
        #[arg(short, long)]
        save: String,
    },
    Complete {
        passphrase: String,

        #[arg(short, long)]
        save: String,
    },
}
