mod cli;
mod config;
mod geyser;
mod transaction;

use anyhow::{Context, Result};
use cli::{Commands, parse_args};
use config::Config;
use std::fs::File;
use std::io::Write;
use tokio::signal;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = parse_args();

    match cli.command {
        Commands::Start {
            config: config_path,
        } => {
            let config = Config::load(&config_path)
                .context(format!("Failed to load config from {:?}", config_path))?;
            info!("Configuration loaded successfully");

            let destination = config.destination_pubkey()?;
            let tx_sender = transaction::TransactionSender::new(
                &config.keypair_path,
                destination,
                config.sol_amount,
                &config.solana_rpc_url,
            )?;

            info!(
                "Transaction sender initialized with destination: {}",
                destination
            );

            // Start geyser subscription
            let mut block_rx = geyser::start_subscription(
                config.geyser_endpoint.clone(),
                config.geyser_token.clone(),
            )
            .await?;

            info!("Started subscription to Yellowstone Geyser, listening for blocks...");

            // Process block notifications
            loop {
                tokio::select! {
                    Some(slot) = block_rx.recv() => {
                        info!("Received new block: slot {}", slot);

                        // Send transaction for new block
                        match tx_sender.send_transaction().await {
                            Ok(signature) => {
                                info!("Transaction sent successfully: {}", signature);
                            }
                            Err(e) => {
                                error!("Failed to send transaction: {}", e);
                            }
                        }
                    }

                    // Handle program termination
                    _ = signal::ctrl_c() => {
                        info!("Received shutdown signal, exiting...");
                        break;
                    }
                }
            }
        }

        Commands::GenerateConfig { output } => {
            let sample_config = r#"# Yellowstone Geyser gRPC configuration
geyser_endpoint: "https://grpc.ny.shyft.to"
geyser_token: "YOUR_GEYSER_TOKEN"

# Solana RPC endpoint for sending transactions
solana_rpc_url: "https://api.mainnet-beta.solana.com"

# Solana transaction configuration
keypair_path: "/path/to/your/keypair.json"
destination_wallet: "YOUR_DESTINATION_WALLET_ADDRESS"
sol_amount: 0.001
"#;

            let mut file = File::create(&output)
                .context(format!("Failed to create config file at {:?}", output))?;
            file.write_all(sample_config.as_bytes())?;

            info!("Sample configuration file generated at {:?}", output);
            info!(
                "Please edit the file with your actual configuration before starting the service."
            );
        }
    }

    Ok(())
}
