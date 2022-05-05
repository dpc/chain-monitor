use std::collections::HashMap;

use super::{
    ChainId::{self, *},
    SourceId::{self, *},
};
use crate::{ChainStateTs, ChainStateUpdate, ChainUpdateRecorder};
use anyhow::Result;
use axum::async_trait;
use tracing::{debug, log::warn};

/// A catch-all of single-chain explorers and alikes
pub struct ChainMonitor {
    client: reqwest::Client,
    url: String,
}

impl ChainMonitor {
    pub fn new(url: String) -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent("curl/7.79.1")
                .build()?,
            url,
        })
    }

    pub async fn get_json(&self) -> Result<HashMap<String, ChainStateTs>> {
        Ok(self
            .client
            .get(format!("{}/state", self.url))
            .send()
            .await?
            .error_for_status()?
            .json::<HashMap<String, ChainStateTs>>()
            .await?)
    }
}

#[async_trait]
impl super::StaticSource for ChainMonitor {
    const ID: SourceId = SourceId::ChainMonitor;
    const SUPPORTED_CHAINS: &'static [ChainId] = &[
        Algorand,
        Avalanche,
        Stacks,
        EthereumClassic,
        Casper,
        Celo,
        Tezos,
    ];

    async fn check_updates(&self, recorder: &dyn ChainUpdateRecorder) {
        match self.get_json().await {
            Err(e) => warn!(
                "Could not get chain state from {}: {e}",
                Self::ID.short_name(),
            ),
            Ok(states) => {
                for (ticker, state) in states {
                    if let Some(chain) = ChainId::from_ticker(&ticker) {
                        recorder
                            .update(ChainStateUpdate {
                                source: ChainMonitor,
                                chain,
                                state: state.state,
                            })
                            .await;
                    } else {
                        debug!("Unknown ticker {ticker} ignored from {}", self.url);
                    }
                }
            }
        }
    }
}
