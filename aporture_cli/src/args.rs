use std::{net::SocketAddr, path::PathBuf};

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
    /// Receive a file
    Receive {
        #[arg(short, long, value_names(["PATH"]))]
        destination: Option<PathBuf>,

        #[command(flatten)]
        method: ReceiveMethod,

        #[arg(short, long, value_names(["NAME"]))]
        save: Option<String>,
    },

    /// Query or modify saved contacts
    Contacts {
        #[command(subcommand)]
        command: ContactCommand,
    },

    /// Pair a new contact
    Pair {
        #[command(subcommand)]
        command: PairCommand,
    },

    // Modify server configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
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
pub struct ReceiveMethod {
    pub passphrase: Option<String>,

    #[arg(short, long, value_names(["NAME"]))]
    pub contact: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum PairCommand {
    Start {
        name: String,
        passphrase: Option<String>,
    },
    Complete {
        name: String,
        passphrase: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum ContactCommand {
    List,
    Delete { name: String },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    Get,
    Set { server_address: SocketAddr },
}
