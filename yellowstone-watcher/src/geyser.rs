use anyhow::Result;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info};
use yellowstone_grpc_client::ClientTlsConfig;
use yellowstone_grpc_client::GeyserGrpcClient;
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestFilterBlocks, subscribe_update::UpdateOneof,
};
use yellowstone_grpc_proto::tonic::codegen::tokio_stream::StreamExt;
use yellowstone_grpc_proto::tonic::service::Interceptor;

pub struct GeyserSubscriber {
    endpoint: String,
    token: String,
}

impl GeyserSubscriber {
    pub fn new(endpoint: String, token: String) -> Self {
        Self { endpoint, token }
    }

    async fn create_client(&self) -> Result<GeyserGrpcClient<impl Interceptor>> {
        info!("Connecting to Yellowstone gRPC endpoint: {}", self.endpoint);

        GeyserGrpcClient::build_from_shared(self.endpoint.clone())?
            .x_token(Some(&self.token))?
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(10))
            .tls_config(ClientTlsConfig::new().with_native_roots())?
            .max_decoding_message_size(1024 * 1024 * 1024)
            .connect()
            .await
            .map_err(Into::into)
    }

    pub async fn subscribe(&self, block_tx: mpsc::Sender<u64>) -> Result<()> {
        // Create client on demand
        let mut client = self.create_client().await?;

        // Create subscription request with block filter
        let mut blocks = HashMap::new();
        blocks.insert("blocks".to_string(), SubscribeRequestFilterBlocks {
            account_include: vec!["11111111111111111111111111111111".to_string()], // just a system program to bypass the filter requirements
            include_transactions: Some(true),
            include_accounts: Some(false),
            include_entries: Some(false),
        });

        // Create subscription request
        let subscribe_request = SubscribeRequest {
            slots: HashMap::new(),
            accounts: HashMap::new(),
            transactions: HashMap::new(),
            blocks,
            blocks_meta: HashMap::new(),
            accounts_data_slice: vec![],
            commitment: Some(CommitmentLevel::Confirmed as i32),
            entry: HashMap::new(),
            transactions_status: HashMap::new(),
            ping: None,
            from_slot: None,
        };

        info!("Subscribing to block updates...");
        let (_, mut subscription_stream) = client
            .subscribe_with_request(Some(subscribe_request))
            .await?;
        info!("Subscription established successfully");

        while let Some(message) = subscription_stream.next().await {
            if let Ok(message) = message {
                match message.update_oneof {
                    Some(UpdateOneof::Block(block)) => {
                        info!("Received block update for slot: {}", block.slot);
                        if let Err(e) = block_tx.send(block.slot).await {
                            error!("Failed to send block update to handler: {}", e);
                        }
                    }
                    _ => {} // Ignore other update types
                }
            } else {
                error!("Error receiving message: {:?}", message.err());
            }
        }

        error!("Subscription stream ended");
        Ok(())
    }
}

pub async fn start_subscription(endpoint: String, token: String) -> Result<mpsc::Receiver<u64>> {
    let (tx, rx) = mpsc::channel(100); // Buffer size of 100
    let subscriber = GeyserSubscriber::new(endpoint, token);

    tokio::spawn(async move {
        if let Err(e) = subscriber.subscribe(tx).await {
            error!("Geyser subscription error: {}", e);
        }
    });

    Ok(rx)
}
