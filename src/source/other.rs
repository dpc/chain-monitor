use super::{
    ChainId::{self, *},
    SourceId::{self, *},
};
use crate::{ChainState, ChainStateUpdate, ChainUpdateRecorder};
use anyhow::{format_err, Result};
use axum::async_trait;
use regex::Regex;
use serde_json::Value;
use tracing::log::warn;

/// A catch-all of single-chain explorers and alikes
pub struct Other {
    client: reqwest::Client,
    rate_limiter: super::UpdateRateLimiter,
}

fn as_not_null(v: &Value) -> Option<&Value> {
    if v.is_null() {
        None
    } else {
        Some(v)
    }
}

impl Other {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent("curl/7.79.1")
                .build()?,
            rate_limiter: super::UpdateRateLimiter::new(<Self as super::StaticSource>::ID),
        })
    }

    pub async fn get_json(&self, url: &str) -> Result<Value> {
        Ok(self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?)
    }

    pub async fn get_chain_state(&self, chain: ChainId) -> Result<ChainState> {
        Ok(match chain {
            ChainId::Algorand => self.get_algorand_chain_state().await?,
            ChainId::Avalanche => self.get_avalanche_chain_state().await?,
            ChainId::Stacks => self.get_stacks_chain_state().await?,
            ChainId::EthereumClassic => self.get_etc_chain_state().await?,
            ChainId::Casper => self.get_casper_chain_state().await?,
            ChainId::Celo => self.get_celo_chain_state().await?,
            ChainId::Tezos => self.get_tezos_chain_state().await?,
            ChainId::HederaHashgraph => self.get_hedera_chain_state().await?,
            _ => unreachable!(),
        })
    }

    pub async fn get_algorand_chain_state(&self) -> Result<ChainState> {
        let value = self
            .get_json("https://indexer.algoexplorerapi.io/v2/blocks?latest=1")
            .await?;

        let last_block = as_not_null(&value["blocks"][0])
            .ok_or_else(|| format_err!("missing last block data"))?;

        Ok(ChainState {
            hash: last_block["hash"]
                .as_str()
                .ok_or_else(|| format_err!("missing hash"))?
                .to_owned(),
            height: last_block["round"]
                .as_u64()
                .ok_or_else(|| format_err!("missing height"))?,
        })
    }

    pub async fn get_avalanche_chain_state(&self) -> Result<ChainState> {
        let body = self
            .client
            .get("https://snowtrace.io/blocks")
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;

        let body = String::from_utf8_lossy(&body).to_owned();

        let regex_hash = Regex::new(r"/block/(0x[a-f0-9]+)").expect("regex incorrect");
        let hash = regex_hash
            .captures_iter(&body)
            .next()
            .ok_or_else(|| format_err!("didn't find block hash"))?;

        let regex_block_num = Regex::new(r"/block/([0-9]+)").expect("regex incorrect");
        let block_number = regex_block_num
            .captures_iter(&body)
            .next()
            .ok_or_else(|| format_err!("didn't find block number"))?;

        Ok(ChainState {
            hash: hash[0].to_owned(),
            height: block_number[1].parse::<u64>()?,
        })
    }

    pub async fn get_stacks_chain_state(&self) -> Result<ChainState> {
        let value = self.get_json(
            "https://stacks-node-api.stacks.co/extended/v1/block?limit=1&offset=0&unanchored=true",
        ).await?;

        let last_block = as_not_null(&value["results"][0])
            .ok_or_else(|| format_err!("missing last block data"))?;

        Ok(ChainState {
            hash: last_block["hash"]
                .as_str()
                .ok_or_else(|| format_err!("missing hash"))?
                .to_owned(),
            height: last_block["height"]
                .as_u64()
                .ok_or_else(|| format_err!("missing height"))?,
        })
    }

    pub async fn get_casper_chain_state(&self) -> Result<ChainState> {
        let value = self.get_json(
            "https://event-store-api-clarity-mainnet.make.services/blocks?page=1&limit=1&order_direction=DESC",
        ).await?;

        let last_block =
            as_not_null(&value["data"][0]).ok_or_else(|| format_err!("missing last block data"))?;

        Ok(ChainState {
            hash: last_block["blockHash"]
                .as_str()
                .ok_or_else(|| format_err!("missing hash"))?
                .to_owned(),
            height: last_block["height"]
                .as_u64()
                .ok_or_else(|| format_err!("missing height"))?,
        })
    }

    pub async fn get_etc_chain_state(&self) -> Result<ChainState> {
        let value = self
            .client
            .get("https://blockscout.com/etc/mainnet/chain-blocks")
            .header("x-requested-with", "XMLHttpRequest")
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;

        let last_block = as_not_null(&value["blocks"][0])
            .ok_or_else(|| format_err!("missing last block data"))?;

        // LOL, WUT
        let some_html_crap = last_block["chain_block_html"]
            .as_str()
            .ok_or_else(|| format_err!("missing hash data"))?;

        let regex_hash = Regex::new(r"(0x[a-f0-9]+)").expect("regex incorrect");
        let hash = regex_hash
            .captures_iter(some_html_crap)
            .next()
            .ok_or_else(|| format_err!("didn't find block hash"))?;

        Ok(ChainState {
            hash: hash[0].to_owned(),
            height: last_block["block_number"]
                .as_u64()
                .ok_or_else(|| format_err!("missing height"))?,
        })
    }

    pub async fn get_celo_chain_state(&self) -> Result<ChainState> {
        let value = self
            .get_json("https://explorer.celo.org/blocks?type=JSON")
            .await?;

        // another html crap; oh well
        let some_html_crap = as_not_null(&value["items"][0])
            .ok_or_else(|| format_err!("missing last block data"))?
            .as_str()
            .ok_or_else(|| format_err!("invalid last block data"))?;

        let regex_hash = Regex::new(r"(0x[a-f0-9]+)").expect("regex incorrect");
        let hash = regex_hash
            .captures_iter(some_html_crap)
            .next()
            .ok_or_else(|| format_err!("didn't find block hash"))?;

        let regex_block_num =
            Regex::new("data-block-number=\"([0-9]+)\"").expect("regex incorrect");
        let block_number = regex_block_num
            .captures_iter(some_html_crap)
            .next()
            .ok_or_else(|| format_err!("didn't find block number"))?;

        Ok(ChainState {
            hash: hash[0].to_owned(),
            height: block_number[1].parse::<u64>()?,
        })
    }

    pub async fn get_hedera_chain_state(&self) -> Result<ChainState> {
        let value = self
            .get_json("https://mainnet-public.mirrornode.hedera.com/api/v1/transactions?limit=1")
            .await?;

        let last_tx = as_not_null(&value["transactions"][0])
            .ok_or_else(|| format_err!("missing last block data"))?;

        Ok(ChainState {
            hash: last_tx["transaction_hash"]
                .as_str()
                .ok_or_else(|| format_err!("missing hash"))?
                .to_owned(),
            height: ((last_tx["consensus_timestamp"]
                .as_str()
                .ok_or_else(|| format_err!("missing height"))?
                .parse::<f64>()?
                - 1596139200f64) / 5.) as u64,
        })
    }
    pub async fn get_tezos_chain_state(&self) -> Result<ChainState> {
        let value = self
            .get_json("https://api.tzstats.com/explorer/tip")
            .await?;

        Ok(ChainState {
            hash: value["block_hash"]
                .as_str()
                .ok_or_else(|| format_err!("missing hash"))?
                .to_owned(),
            height: value["height"]
                .as_u64()
                .ok_or_else(|| format_err!("missing height"))?,
        })
    }
}

#[async_trait]
impl super::StaticSource for Other {
    const ID: SourceId = SourceId::Other;
    const SUPPORTED_CHAINS: &'static [ChainId] = &[
        Algorand,
        Avalanche,
        Casper,
        Celo,
        EthereumClassic,
        HederaHashgraph,
        Stacks,
        Tezos,
    ];

    async fn check_updates(&self, recorder: &dyn ChainUpdateRecorder) {
        for &chain in Self::SUPPORTED_CHAINS {
            if self.rate_limiter.should_check(chain, recorder).await {
                match self.get_chain_state(chain).await {
                    Err(e) => warn!(
                        "Could not get chain state from {} for {}: {e}",
                        Self::ID.short_name(),
                        chain.short_name()
                    ),
                    Ok(state) => {
                        recorder
                            .update(ChainStateUpdate {
                                source: Other,
                                chain: chain,
                                state,
                            })
                            .await;
                    }
                }
            }
        }
    }
}
