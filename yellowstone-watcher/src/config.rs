use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::fs::File;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub geyser_endpoint: String,
    pub geyser_token: String,
    pub keypair_path: String,
    pub destination_wallet: String,
    pub sol_amount: f64,
    pub solana_rpc_url: String,
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path).context("Failed to open config file")?;
        let config: Config =
            serde_yaml::from_reader(file).context("Failed to parse config file")?;
        Ok(config)
    }

    pub fn destination_pubkey(&self) -> Result<Pubkey> {
        self.destination_wallet
            .parse::<Pubkey>()
            .context("Invalid destination wallet pubkey in config")
    }
}
