use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author = "Vladislav Dyachenko")]
#[command(version = "0.1.0")]
#[command(about = "Solana bulk transfer utility")]
#[command(
    long_about = "A utility for executing multiple SOL transfers in parallel. \
    Supports configurable concurrency, timeout settings, and batch processing from YAML configuration."
)]
pub struct Args {
    /// Path to the YAML configuration file
    #[clap(short, long, default_value = "config.yaml")]
    pub config: PathBuf,

    /// Maximum number of concurrent transfers
    #[clap(long, default_value = "10")]
    pub concurrent: usize,

    /// Timeout in seconds for transaction confirmation
    #[clap(short, long, default_value = "60")]
    pub timeout: u64,
}
