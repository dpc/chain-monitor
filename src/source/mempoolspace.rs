use super::{ChainId, ChainId::*, SourceId};
use crate::{ChainState, ChainStateUpdate, ChainUpdateRecorder};
use anyhow::{bail, Result};
use axum::async_trait;
use rand::{seq::SliceRandom, thread_rng};
use serde::Deserialize;

#[derive(Deserialize)]
struct Block {
    id: String,
    height: u64,
}
pub(crate) async fn get_chain_state(
    client: &reqwest::Client,
    chain_prefix: &str,
) -> Result<ChainState> {
    let resp = client
        .get(format!("https://mempool.space/{chain_prefix}api/blocks/"))
        .send()
        .await?
        .error_for_status()?
        .json::<Vec<Block>>()
        .await?;

    if resp.is_empty() {
        bail!("No blocks returned");
    }

    Ok(ChainState {
        height: resp[0].height,
        hash: resp[0].id.clone(),
    })
}

async fn get_updates(
    client: &reqwest::Client,
    chain: ChainId,
    chain_prefix: &str,
) -> Option<ChainStateUpdate> {
    match get_chain_state(client, chain_prefix).await {
        Ok(state) => Some(ChainStateUpdate {
            source: SourceId::MempoolSpace,
            chain: chain.into(),
            state,
        }),
        Err(e) => {
            let chain_name: &str = chain.into();
            tracing::warn!("Couldn't update MempoolSpace  {chain_name}: {e}");
            None
        }
    }
}

pub struct MempoolSpace {
    client: reqwest::Client,
}

impl MempoolSpace {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent("curl/7.79.1")
                .build()?,
        })
    }

    fn get_api_prefix_for_chain(chain: ChainId) -> &'static str {
        match chain {
            Bitcoin => "",
            BitcoinTestnet => "testnet/",
            BitcoinSignet => "signet/",
            _ => unreachable!(),
        }
    }
}

#[async_trait]
impl super::StaticSource for MempoolSpace {
    const ID: SourceId = SourceId::MempoolSpace;
    const SUPPORTED_CHAINS: &'static [ChainId] = &[Bitcoin, BitcoinTestnet, BitcoinSignet];

    async fn check_updates(&self, recorder: &dyn ChainUpdateRecorder) {
        // randomize the order to give all chains a chance, even in the presence
        // of rate limiting
        let mut supported_chains = Self::SUPPORTED_CHAINS.to_vec();
        supported_chains.shuffle(&mut thread_rng());

        for chain_id in supported_chains {
            if let Some(update) = get_updates(
                &self.client,
                chain_id,
                Self::get_api_prefix_for_chain(chain_id),
            )
            .await
            {
                recorder.update(update).await;
            }
        }
    }
}
