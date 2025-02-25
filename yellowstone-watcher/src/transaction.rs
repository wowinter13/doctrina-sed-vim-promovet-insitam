use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{Keypair, Signer, read_keypair_file},
    system_instruction,
    transaction::Transaction,
};
use std::path::Path;
use tracing::{debug, info};

pub struct TransactionSender {
    keypair: Keypair,
    rpc_client: RpcClient,
    destination: Pubkey,
    lamports: u64,
}

impl TransactionSender {
    pub fn new<P: AsRef<Path>>(
        keypair_path: P,
        destination: Pubkey,
        sol_amount: f64,
        rpc_url: &str,
    ) -> Result<Self> {
        let keypair = read_keypair_file(keypair_path)
            .map_err(|e| anyhow::anyhow!("Failed to read keypair file: {}", e))?;

        // Convert SOL to lamports (1 SOL = 10^9 lamports)
        let lamports = (sol_amount * 1_000_000_000.0) as u64;

        let rpc_client =
            RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

        Ok(Self {
            keypair,
            rpc_client,
            destination,
            lamports,
        })
    }

    pub async fn send_transaction(&self) -> Result<String> {
        debug!(
            "Preparing to send {} lamports to {}",
            self.lamports, self.destination
        );

        let balance = self.rpc_client.get_balance(&self.keypair.pubkey())?;
        if balance < self.lamports {
            return Err(anyhow::anyhow!(
                "Insufficient balance: {} SOL (need at least {} SOL)",
                balance as f64 / 1_000_000_000.0,
                self.lamports as f64 / 1_000_000_000.0
            ));
        }

        let instruction =
            system_instruction::transfer(&self.keypair.pubkey(), &self.destination, self.lamports);

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.keypair.pubkey()),
            &[&self.keypair],
            recent_blockhash,
        );

        info!(
            "Sending {} SOL from {} to {}",
            self.lamports as f64 / 1_000_000_000.0,
            self.keypair.pubkey(),
            self.destination
        );

        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;

        info!("Transaction confirmed with signature: {}", signature);
        Ok(signature.to_string())
    }
}
