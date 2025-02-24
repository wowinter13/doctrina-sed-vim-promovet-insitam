use futures::future::join_all;
use serde::{Deserialize, Serialize};
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use std::error::Error;
use std::fs;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    wallets: Vec<String>,
    #[serde(default = "default_batch_size")]
    batch_size: usize,
    #[serde(default = "default_rpc_url")]
    rpc_url: String,
}

fn default_batch_size() -> usize {
    25
}

fn default_rpc_url() -> String {
    "https://api.mainnet-beta.solana.com".to_string()
}

#[derive(Debug, Clone)]
struct WalletBalance {
    address: String,
    balance_sol: f64,
    fetch_time_ms: u64,
}

// Load and validate config
async fn read_config(config_path: &str) -> Result<Config, Box<dyn Error>> {
    let contents = fs::read_to_string(config_path)?;
    let config: Config = serde_yaml::from_str(&contents)?;

    // Check for empty wallet list
    if config.wallets.is_empty() {
        return Err("No wallet addresses specified in config".into());
    }

    // Validate pubkeys upfront
    for addr in &config.wallets {
        Pubkey::from_str(addr).map_err(|e| format!("Invalid pubkey {}: {}", addr, e))?;
    }

    Ok(config)
}

// Fetch single wallet balance
async fn fetch_wallet_balance(
    client: Arc<RpcClient>,
    address: String,
) -> Result<WalletBalance, String> {
    let start_time = Instant::now();
    let pubkey = Pubkey::from_str(&address).map_err(|e| e.to_string())?;
    // Include address in RPC error for clarity
    let balance = client
        .get_balance(&pubkey)
        .map_err(|e| format!("RPC error for {}: {}", address, e))?;
    let elapsed = start_time.elapsed().as_millis() as u64;

    Ok(WalletBalance {
        address,
        balance_sol: balance as f64 / 1_000_000_000.0,
        fetch_time_ms: elapsed,
    })
}

// Fetch balances in batches
async fn fetch_wallet_balances(config_path: &str) -> Result<Vec<WalletBalance>, Box<dyn Error>> {
    let config = read_config(config_path).await?;
    println!("Loading {} wallet addresses", config.wallets.len());

    // Added timeout to avoid hanging RPC calls
    let client = Arc::new(RpcClient::new_with_timeout_and_commitment(
        config.rpc_url,
        Duration::from_secs(30),
        CommitmentConfig::confirmed(),
    ));

    let mut all_results = Vec::new();
    let total_start = Instant::now();

    for (batch_idx, chunk) in config.wallets.chunks(config.batch_size).enumerate() {
        println!(
            "Processing batch {} ({} addresses)",
            batch_idx + 1,
            chunk.len()
        );
        let batch_start = Instant::now();

        // Parallel tasks for each wallet in batch
        let tasks: Vec<_> = chunk
            .iter()
            .map(|addr| {
                let client_clone = Arc::clone(&client);
                let addr_clone = addr.clone();
                tokio::spawn(async move { fetch_wallet_balance(client_clone, addr_clone).await })
            })
            .collect();

        let results = join_all(tasks).await;

        // Log errors, collect successes
        for result in results {
            match result {
                Ok(Ok(balance)) => all_results.push(balance),
                Ok(Err(e)) => eprintln!("Failed to fetch balance: {}", e),
                Err(e) => eprintln!("Task panicked: {}", e),
            }
        }

        println!(
            "Batch {} completed in {:.2}s",
            batch_idx + 1,
            batch_start.elapsed().as_secs_f64()
        );

        // Delay to avoid rate limits—semaphores too complex for this
        if batch_idx < config.wallets.chunks(config.batch_size).len() - 1 {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    println!(
        "Fetched all balances in {:.2}s",
        total_start.elapsed().as_secs_f64()
    );

    Ok(all_results)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config_path = "config.yaml";
    let balances = fetch_wallet_balances(config_path).await?;

    println!("\nWallet Balance Results:");
    println!(
        "{:<44} | {:<15} | {:<8}",
        "Address", "Balance (SOL)", "Time (ms)"
    );
    println!("{}", "-".repeat(75));

    for balance in &balances {
        println!(
            "{:<44} | {:<15.5} | {:<8}",
            balance.address, balance.balance_sol, balance.fetch_time_ms
        );
    }

    println!("\nSummary: Fetched {} balances", balances.len());
    Ok(())
}
