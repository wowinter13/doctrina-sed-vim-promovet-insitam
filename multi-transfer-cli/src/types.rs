use serde::Deserialize;
use solana_sdk::signature::Signature;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub rpc_url: String,
    pub amount: f64,
    pub source_wallets: Vec<SourceWallet>,
    pub destination_wallets: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SourceWallet {
    pub from_keypair_path: String,
    pub amount: Option<f64>,
}

#[derive(Debug)]
pub struct TransferSpec {
    pub from_keypair_path: String,
    pub to_address: String,
    pub amount_sol: f64,
}

#[derive(Debug)]
pub struct TransferResult {
    pub from: String,
    pub to: String,
    pub amount: f64,
    pub signature: Signature,
    pub duration_ms: u64,
    pub status: TransferStatus,
}

#[derive(Debug)]
pub enum TransferStatus {
    Success,
    Failed(String),
    Timeout,
}
