use crate::{get_now_ts, ChainState, ChainStateUpdate, ChainUpdateRecorder};
use anyhow::{bail, Result};
use axum::async_trait;
use serde::Deserialize;

use super::{ChainId, ChainId::*, SourceId};

#[derive(Deserialize)]
struct BlocksBody {
    data: Vec<BlocksDataItem>,
}

#[derive(Deserialize)]
struct BlocksDataItem {
    hash: String,
    height: u64,
}

pub(crate) async fn get_chain_state(
    client: &reqwest::Client,
    chain_api_symbol: &str,
) -> Result<ChainState> {
    let resp = client
        .get(format!(
            "https://blockchain.coinmarketcap.com/api/blocks?symbol={chain_api_symbol}&start=1&limit=1&quote=true"
        ))
        .send()
        .await?
        .error_for_status()?
        .json::<BlocksBody>()
        .await?;

    if let Some(item) = resp.data.get(0) {
        Ok(ChainState {
            ts: get_now_ts(),
            hash: item.hash.clone(),
            height: item.height,
        })
    } else {
        bail!("No blocks returned");
    }
}

async fn get_chain_update(
    client: &reqwest::Client,
    chain: ChainId,
    chain_api_symbol: &str,
) -> Option<ChainStateUpdate> {
    match get_chain_state(client, chain_api_symbol).await {
        Ok(state) => Some(ChainStateUpdate {
            source: SourceId::CMC,
            chain: chain.into(),
            state,
        }),
        Err(e) => {
            let chain_name: &str = chain.into();
            tracing::warn!("Couldn't update CoinMarketCap {chain_name}: {e}");
            None
        }
    }
}

pub struct CoinMarketCap {
    client: reqwest::Client,
}

impl CoinMarketCap {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent("curl/7.79.1")
                .build()?,
        })
    }

    fn coin_symbol_for_chain(chain: ChainId) -> &'static str {
        match chain {
            Bitcoin => "BTC",
            BinanceCoin => "BNB",
            Ethereum => "ETH",
            Litecoin => "LTC",

            _ => unreachable!(),
        }
    }
}

#[async_trait]
impl super::StaticSource for CoinMarketCap {
    const ID: SourceId = SourceId::CMC;
    const SUPPORTED_CHAINS: &'static [ChainId] = &[Bitcoin, Ethereum, Litecoin, BinanceCoin];

    async fn check_updates(&self, recorder: &dyn ChainUpdateRecorder) {
        for &chain_id in Self::SUPPORTED_CHAINS {
            if let Some(update) = get_chain_update(
                &self.client,
                chain_id,
                Self::coin_symbol_for_chain(chain_id),
            )
            .await
            {
                recorder.update(update).await;
            }
        }
    }
}
