use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(author = "Vladislav Dyachenko")]
#[command(version = "0.1.0")]
#[command(
    about = "A service that monitors Solana blocks via Yellowstone gRPC and sends transactions"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start watching for new blocks and sending transactions
    Start {
        /// Path to config.yaml file
        #[arg(short, long, default_value = "config.yaml")]
        config: PathBuf,
    },

    /// Generate a sample config file
    GenerateConfig {
        /// Path to output config file
        #[arg(short, long, default_value = "config.yaml")]
        output: PathBuf,
    },
}

pub fn parse_args() -> Cli {
    Cli::parse()
}
