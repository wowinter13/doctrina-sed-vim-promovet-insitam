mod args;
mod types;

use anyhow::{Context, Result};
use args::Args;
use clap::Parser;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::Signature,
    signer::{Signer, keypair::read_keypair_file},
    system_instruction,
    transaction::Transaction,
};
use std::{
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{sync::Semaphore, time::sleep};
use tracing::{info, warn};
use types::{Config, TransferResult, TransferSpec, TransferStatus};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let args = Args::parse();

    // Read the configuration file
    let config_data = std::fs::read_to_string(&args.config)
        .with_context(|| format!("Failed to read config file: {:?}", args.config))?;

    let config: Config =
        serde_yaml::from_str(&config_data).context("Failed to parse config file")?;

    // Create RPC client
    let rpc_client = Arc::new(RpcClient::new_with_commitment(
        config.rpc_url.clone(),
        CommitmentConfig::confirmed(),
    ));

    // Get recent blockhash
    let recent_blockhash = rpc_client
        .get_latest_blockhash()
        .await
        .context("Failed to get recent blockhash")?;

    // Generate all transfer combinations
    let mut transfers = Vec::new();
    for source in &config.source_wallets {
        let amount = source.amount.unwrap_or(config.amount); // if amount is not provided, use default amount
        for dest in &config.destination_wallets {
            transfers.push(TransferSpec {
                from_keypair_path: source.from_keypair_path.clone(),
                to_address: dest.clone(),
                amount_sol: amount,
            });
        }
    }

    info!("Generated {} transfers from configuration", transfers.len());

    // Set up semaphore for controlling concurrency
    let semaphore = Arc::new(Semaphore::new(args.concurrent));

    // Execute transfers in parallel
    let start_time = Instant::now();
    let transfer_futures = transfers.iter().map(|transfer| {
        let rpc_client = rpc_client.clone();
        let semaphore = semaphore.clone();

        let keypair_path = transfer.from_keypair_path.clone();
        let to_address = transfer.to_address.clone();
        let amount_sol = transfer.amount_sol;
        let recent_blockhash = recent_blockhash;
        let timeout = args.timeout;

        async move {
            // Acquire permit from semaphore
            let _permit = semaphore.acquire().await.unwrap();

            info!("Starting transfer: {} -> {}", keypair_path, to_address);

            // Load keypair
            let from_keypair = match read_keypair_file(&keypair_path) {
                Ok(kp) => kp,
                Err(e) => {
                    warn!("Failed: Keypair loading error: {}", e);
                    return TransferResult {
                        from: keypair_path,
                        to: to_address,
                        amount: amount_sol,
                        signature: Signature::default(),
                        duration_ms: 0,
                        status: TransferStatus::Failed(format!("Keypair loading error: {}", e)),
                    };
                }
            };

            let from_pubkey = from_keypair.pubkey();

            // Parse destination address
            let to_pubkey = match Pubkey::from_str(&to_address) {
                Ok(pk) => pk,
                Err(e) => {
                    warn!("Failed: Invalid destination address: {}", e);
                    return TransferResult {
                        from: from_pubkey.to_string(),
                        to: to_address,
                        amount: amount_sol,
                        signature: Signature::default(),
                        duration_ms: 0,
                        status: TransferStatus::Failed(format!(
                            "Invalid destination address: {}",
                            e
                        )),
                    };
                }
            };

            info!("Creating transaction from {} to {}", from_pubkey, to_pubkey);

            // Convert SOL to lamports
            let lamports = (amount_sol * 1_000_000_000.0) as u64;

            // Create transfer instruction
            let instruction = system_instruction::transfer(&from_pubkey, &to_pubkey, lamports);

            // Create transaction
            let tx = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&from_pubkey),
                &[&from_keypair],
                recent_blockhash,
            );

            info!("Sending transaction...");

            // Send the transaction
            let start = Instant::now();
            let signature = match rpc_client
                .send_transaction_with_config(&tx, RpcSendTransactionConfig {
                    skip_preflight: false,
                    preflight_commitment: Some(CommitmentConfig::confirmed().commitment),
                    encoding: None,
                    max_retries: Some(5),
                    min_context_slot: None,
                })
                .await
            {
                Ok(sig) => sig,
                Err(e) => {
                    warn!("Failed to send transaction: {}", e);
                    return TransferResult {
                        from: from_pubkey.to_string(),
                        to: to_pubkey.to_string(),
                        amount: amount_sol,
                        signature: Signature::default(),
                        duration_ms: start.elapsed().as_millis() as u64,
                        status: TransferStatus::Failed(format!("Send error: {}", e)),
                    };
                }
            };

            info!("Confirming transaction: {}", signature);

            // Wait for confirmation
            let timeout_duration = Duration::from_secs(timeout);
            let mut status_result = None;
            let end_time = Instant::now() + timeout_duration;

            while Instant::now() < end_time {
                match rpc_client.get_signature_status(&signature).await {
                    Ok(Some(status)) => {
                        status_result = Some(status);
                        break;
                    }
                    Ok(None) => {
                        sleep(Duration::from_millis(500)).await;
                    }
                    Err(e) => {
                        warn!("Error checking signature status: {}", e);
                        sleep(Duration::from_millis(1000)).await;
                    }
                }
            }

            let duration_ms = start.elapsed().as_millis() as u64;

            let status = match status_result {
                Some(Ok(())) => {
                    info!("Success: {} in {}ms", signature, duration_ms);
                    TransferStatus::Success
                }
                Some(Err(e)) => {
                    warn!("Failed: {}", e);
                    TransferStatus::Failed(format!("Transaction error: {:?}", e))
                }
                None => {
                    warn!("Timeout while confirming transaction");
                    TransferStatus::Timeout
                }
            };

            TransferResult {
                from: from_pubkey.to_string(),
                to: to_pubkey.to_string(),
                amount: amount_sol,
                signature,
                duration_ms,
                status,
            }
        }
    });

    // Collect all futures and execute them
    let handles: Vec<_> = transfer_futures.map(tokio::spawn).collect();

    let mut results = Vec::new();
    let mut completed = 0;
    let total = handles.len();

    for handle in handles {
        let result = handle.await?;
        results.push(result);
        completed += 1;
        info!("Progress: {}/{} transfers completed", completed, total);
    }

    info!(
        "All transfers completed in {}ms",
        start_time.elapsed().as_millis()
    );

    // Display results
    println!("\n{:-^80}", " RESULTS SUMMARY ");
    println!(
        "{:<5} {:<12} {:<44} {:<10} {:<10} {:<20} {:<20}",
        "No.", "Status", "Signature", "Amount", "Time (ms)", "From", "To"
    );
    println!("{:-^80}", "");

    let mut success_count = 0;
    let mut failed_count = 0;
    let mut timeout_count = 0;
    let mut total_duration = 0;

    for (i, result) in results.iter().enumerate() {
        let status_str = match &result.status {
            TransferStatus::Success => {
                success_count += 1;
                "SUCCESS"
            }
            TransferStatus::Failed(err) => {
                failed_count += 1;
                println!("    Error details: {}", err);
                "FAILED"
            }
            TransferStatus::Timeout => {
                timeout_count += 1;
                "TIMEOUT"
            }
        };

        total_duration += result.duration_ms;

        println!(
            "{:<5} {:<12} {:<44} {:<10.4} {:<10} {:<20} {:<20}",
            i + 1,
            status_str,
            result.signature.to_string(),
            result.amount,
            result.duration_ms,
            result.from,
            result.to
        );
    }

    // Print summary statistics
    println!("\n{:-^80}", " STATISTICS ");
    println!("Total transfers: {}", results.len());
    println!("Successful: {}", success_count);
    println!("Failed: {}", failed_count);
    println!("Timeouts: {}", timeout_count);
    println!(
        "Average duration: {}ms",
        if !results.is_empty() {
            total_duration / results.len() as u64
        } else {
            0
        }
    );
    println!(
        "Total execution time: {}ms",
        start_time.elapsed().as_millis()
    );

    Ok(())
}
