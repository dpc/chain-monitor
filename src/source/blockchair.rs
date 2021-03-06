use std::collections::HashMap;

use super::{
    ChainId::{self, *},
    SourceId::{self, *},
};
use crate::{ChainState, ChainStateUpdate, ChainUpdateRecorder};
use anyhow::Result;
use axum::async_trait;
use serde::Deserialize;

// TODO: find a nicer way; whoever made this API scheme, really love nesting shit and the word "data"
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
struct HomepageEnBody {
    data: HomepageEnData,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
struct HomepageEnData {
    stats: HomepageEnStats,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
struct HomepageEnStats {
    data: HashMap<String, HomepageEnCoin>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
struct HomepageEnCoin {
    data: Option<HomepageEnCoinData>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
struct HomepageEnCoinData {
    #[serde(alias = "best_ledger_height", alias = "best_snapshot_height")]
    best_block_height: Option<u64>,
    #[serde(alias = "best_ledger_hash", alias = "best_snapshot_hash")]
    best_block_hash: Option<String>,

    best_slot_absolute: Option<u64>,
}

async fn get_homepage_en(client: &reqwest::Client) -> Result<HomepageEnBody> {
    Ok(client
        .get("https://api.blockchair.com/internal/homepage/en")
        .send()
        .await?
        .error_for_status()?
        .json::<HomepageEnBody>()
        .await?)
}

pub struct Blockchair {
    client: reqwest::Client,
}

impl Blockchair {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent("curl/7.79.1")
                .build()?,
        })
    }

    fn coin_symbol_for_chain(chain: ChainId) -> &'static str {
        match chain {
            Bitcoin => "bitcoin",
            BitcoinCash => "bitcoin-cash",
            Ethereum => "ethereum",
            Litecoin => "litecoin",
            BitcoinSV => "bitcoin-sv",
            Doge => "dogecoin",
            Dash => "dash",
            Ripple => "ripple",
            Groestlcoin => "groestlcoin",
            Stellar => "stellar",
            Monero => "monero",
            Cardano => "cardano",
            ZCash => "zcash",
            Mixin => "mixin",
            Eos => "eos",
            ECash => "ecash",
            Polkadot => "polkadot",
            Solana => "solana",
            Kusama => "kusama",

            _ => unreachable!(),
        }
    }
}

#[async_trait]
impl super::StaticSource for Blockchair {
    const ID: SourceId = SourceId::Blockchair;
    const SUPPORTED_CHAINS: &'static [ChainId] = &[
        Bitcoin,
        Ethereum,
        Litecoin,
        Cardano,
        Ripple,
        Ripple,
        Polkadot,
        Doge,
        Solana,
        BitcoinCash,
        Stellar,
        Monero,
        Eos,
        Kusama,
        BitcoinSV,
        ECash,
        Dash,
        Mixin,
        Groestlcoin,
        ZCash,
    ];

    async fn check_updates(&self, recorder: &dyn ChainUpdateRecorder) {
        match get_homepage_en(&self.client).await {
            Ok(state) => {
                let data = state.data.stats.data;

                for &chain in Self::SUPPORTED_CHAINS {
                    let symbol = Self::coin_symbol_for_chain(chain);

                    if let Some(data) = data.get(symbol) {
                        if let Some(data) = data.data.as_ref() {
                            if chain == ChainId::Solana || chain == ChainId::SolanaTestnet {
                                // report slots instead of block height for Solana
                                if let Some(height) = data.best_slot_absolute {
                                    recorder
                                        .update(ChainStateUpdate {
                                            source: Blockchair,
                                            chain: chain,
                                            state: ChainState {
                                                hash: data
                                                    .best_block_hash
                                                    .clone()
                                                    .unwrap_or_else(|| height.to_string()),
                                                height,
                                            },
                                        })
                                        .await;
                                } else {
                                    tracing::warn!(
                                        "Missing chain data for blockchair coin data: {symbol}"
                                    );
                                }
                            } else {
                                if let Some(height) = data.best_block_height {
                                    recorder
                                        .update(ChainStateUpdate {
                                            source: Blockchair,
                                            chain: chain,
                                            state: ChainState {
                                                hash: data
                                                    .best_block_hash
                                                    .clone()
                                                    .unwrap_or_else(|| height.to_string()),
                                                height,
                                            },
                                        })
                                        .await;
                                } else {
                                    tracing::warn!(
                                        "Missing chain data for blockchair coin data: {symbol}"
                                    );
                                }
                            }
                        } else {
                            tracing::warn!("Malformed data for blockchair coin data: {symbol}");
                        }
                    } else {
                        tracing::warn!("Couldn't find blockchair coin data: {symbol}");
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Couldn't update Blockchair: {e}");
            }
        }
    }
}
