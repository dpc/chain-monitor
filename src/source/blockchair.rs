use super::{
    ChainId::{self, *},
    SourceId::{self, *},
};
use crate::{get_now_ts, ChainState, ChainStateUpdate};
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
    data: HomepageEnDataInner,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct HomepageEnDataInner {
    bitcoin: HomepageEnCoin,
    bitcoin_cash: HomepageEnCoin,
    ethereum: HomepageEnCoin,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
struct HomepageEnCoin {
    data: HomepageEnCoinData,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
struct HomepageEnCoinData {
    best_block_height: u64,
    best_block_hash: String,
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
}

#[async_trait]
impl super::StaticSource for Blockchair {
    const ID: SourceId = SourceId::Blockchair;
    const SUPPORTED_CHAINS: &'static [ChainId] = &[
        Btc,
        Eth,
        Ltc,
        Cardano,
        Xrp,
        Xrp,
        Dot,
        Doge,
        Sol,
        Bch,
        Xlm,
        Xmr,
        Eos,
        Kusama,
        Bsv,
        ECash,
        Dash,
        Mixin,
        Groestlcoin,
    ];

    async fn get_updates(&self) -> Vec<ChainStateUpdate> {
        let ts = get_now_ts();
        match get_homepage_en(&self.client).await {
            Ok(state) => {
                vec![
                    ChainStateUpdate {
                        source: Blockchair,
                        chain: Btc,
                        state: ChainState {
                            ts,
                            hash: state.data.stats.data.bitcoin.data.best_block_hash,
                            height: state.data.stats.data.bitcoin.data.best_block_height,
                        },
                    },
                    ChainStateUpdate {
                        source: Blockchair,
                        chain: Bch,
                        state: ChainState {
                            ts,
                            hash: state.data.stats.data.bitcoin_cash.data.best_block_hash,
                            height: state.data.stats.data.bitcoin_cash.data.best_block_height,
                        },
                    },
                    ChainStateUpdate {
                        source: Blockchair,
                        chain: Eth,
                        state: ChainState {
                            ts,
                            hash: state.data.stats.data.ethereum.data.best_block_hash,
                            height: state.data.stats.data.ethereum.data.best_block_height,
                        },
                    },
                ]
            }
            Err(e) => {
                tracing::warn!("Couldn't update Blockchair: {e}");
                vec![]
            }
        }
    }
}
