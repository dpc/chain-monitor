use super::{ChainId, ChainId::*, SourceId};
use crate::ChainUpdateRecorder;
use anyhow::Result;
use axum::async_trait;
use rand::{seq::SliceRandom, thread_rng};

pub struct BitGoV1 {
    client: reqwest::Client,
    rate_limiter: super::UpdateRateLimiter,
}

impl BitGoV1 {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent("curl/7.79.1")
                .build()?,
            rate_limiter: super::UpdateRateLimiter::new(<Self as super::StaticSource>::ID),
        })
    }

    fn host_for_chain(chain: ChainId) -> &'static str {
        match chain {
            Bitcoin => "bitgo.com",
            BitcoinTestnet => "bitgo-test.com",
            _ => {
                unreachable!()
            }
        }
    }

    fn coin_symbol_for_chain(chain: ChainId) -> &'static str {
        match chain {
            Bitcoin => "btc",
            BitcoinTestnet => "tbtc",
            _ => unreachable!(),
        }
    }
}

#[async_trait]
impl super::StaticSource for BitGoV1 {
    const ID: SourceId = SourceId::BitGoV1;
    const SUPPORTED_CHAINS: &'static [ChainId] = &[Bitcoin, BitcoinTestnet];

    async fn check_updates(&self, recorder: &dyn ChainUpdateRecorder) {
        // randomize the order to give all chains a chance, even in the presence
        // of rate limiting
        let mut supported_chains = Self::SUPPORTED_CHAINS.to_vec();
        supported_chains.shuffle(&mut thread_rng());

        for chain_id in supported_chains {
            if self.rate_limiter.should_check(chain_id, recorder).await {
                if let Some(update) = super::bitgo::get_updates(
                    &self.client,
                    chain_id,
                    super::bitgo::BitgoAPI::V1,
                    Self::host_for_chain(chain_id),
                    Self::coin_symbol_for_chain(chain_id),
                )
                .await
                {
                    recorder.update(update).await;
                }
            }
        }
    }
}
