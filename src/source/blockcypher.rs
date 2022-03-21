use super::{ChainId, ChainId::*, SourceId};
use crate::{ChainState, ChainStateUpdate, ChainUpdateRecorder};
use anyhow::Result;
use axum::async_trait;
use rand::{seq::SliceRandom, thread_rng};
use serde::Deserialize;

#[derive(Deserialize)]
struct BlockLatestBody {
    hash: String,
    height: u64,
}

pub(crate) async fn get_chain_state(
    client: &reqwest::Client,
    chain_api_symbol: &str,
) -> Result<ChainState> {
    let resp = client
        .get(format!("https://api.blockcypher.com/v1/{chain_api_symbol}"))
        .send()
        .await?
        .error_for_status()?
        .json::<BlockLatestBody>()
        .await?;

    Ok(ChainState {
        hash: resp.hash,
        height: resp.height,
    })
}

async fn get_updates(
    client: &reqwest::Client,
    chain: ChainId,
    chain_api_symbol: &str,
) -> Option<ChainStateUpdate> {
    match get_chain_state(client, chain_api_symbol).await {
        Ok(state) => Some(ChainStateUpdate {
            source: SourceId::BlockCypher,
            chain: chain.into(),
            state,
        }),
        Err(e) => {
            let chain_name: &str = chain.into();
            tracing::warn!("Couldn't update BlockCypher {chain_name}: {e}");
            None
        }
    }
}

pub struct BlockCypher {
    client: reqwest::Client,
}

impl BlockCypher {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent("curl/7.79.1")
                .build()?,
        })
    }

    fn coin_symbol_for_chain(chain: ChainId) -> &'static str {
        match chain {
            Bitcoin => "btc/main",
            Litecoin => "ltc/main",
            Dash => "dash/main",
            Doge => "doge/main",
            BitcoinTestnet => "btc/test3",
            _ => unreachable!(),
        }
    }
}

#[async_trait]
impl super::StaticSource for BlockCypher {
    const ID: SourceId = SourceId::BlockCypher;
    const SUPPORTED_CHAINS: &'static [ChainId] = &[Bitcoin, Litecoin, Dash, Doge, BitcoinTestnet];

    async fn check_updates(&self, recorder: &dyn ChainUpdateRecorder) {
        // randomize the order to give all chains a chance, even in the presence
        // of rate limiting
        let mut supported_chains = Self::SUPPORTED_CHAINS.to_vec();
        supported_chains.shuffle(&mut thread_rng());

        for chain_id in supported_chains {
            if let Some(update) = get_updates(
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
