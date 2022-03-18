use super::{ChainId, ChainId::*, SourceId};
use crate::{get_now_ts, ChainState, ChainStateUpdate, ChainUpdateRecorder};
use anyhow::{bail, Result};
use axum::async_trait;
use serde::Deserialize;

#[derive(Deserialize)]
struct BlockLatestBody {
    hash: String,
    height: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlocksV2Body {
    block_headers: Vec<BlockV2>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlockV2 {
    hash: String,
    #[serde(deserialize_with = "crate::util::deserialize_number_from_string")]
    number: u64,
}
pub(crate) async fn get_chain_state_v2(
    client: &reqwest::Client,
    chain_api_symbol: &str,
) -> Result<ChainState> {
    let resp = client
        .get(format!(
            "https://api.blockchain.info/v2/{chain_api_symbol}/data/blocks?size=1"
        ))
        .send()
        .await?
        .error_for_status()?
        .json::<BlocksV2Body>()
        .await?;

    if resp.block_headers.len() != 1 {
        bail!(
            "Wrong size of blockHeaders in response: {}",
            resp.block_headers.len()
        );
    }

    Ok(ChainState {
        ts: get_now_ts(),
        hash: resp.block_headers[0].hash.clone(),
        height: resp.block_headers[0].number,
    })
}

pub(crate) async fn get_chain_state_v1(
    client: &reqwest::Client,
    chain_api_symbol: &str,
) -> Result<ChainState> {
    let resp = client
        .get(format!(
            "https://api.blockchain.info/haskoin-store/{chain_api_symbol}/block/best?notx=true"
        ))
        .send()
        .await?
        .error_for_status()?
        .json::<BlockLatestBody>()
        .await?;

    Ok(ChainState {
        ts: get_now_ts(),
        hash: resp.hash,
        height: resp.height,
    })
}
async fn check_chain_update(
    recorer: &dyn ChainUpdateRecorder,
    client: &reqwest::Client,
    chain: ChainId,
    chain_api_symbol: &str,
) {
    let res = if chain == Ethereum {
        get_chain_state_v2(client, chain_api_symbol).await
    } else {
        get_chain_state_v1(client, chain_api_symbol).await
    };

    match res {
        Ok(state) => {
            recorer
                .update(ChainStateUpdate {
                    source: SourceId::Blockchain,
                    chain: chain.into(),
                    state,
                })
                .await
        }
        Err(e) => {
            let chain_name: &str = chain.into();
            tracing::warn!("Couldn't update Blockchain {chain_name}: {e}");
        }
    }
}

pub struct Blockchain {
    client: reqwest::Client,
}

impl Blockchain {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent("curl/7.79.1")
                .build()?,
        })
    }

    fn coin_symbol_for_chain(chain: ChainId) -> &'static str {
        match chain {
            BitcoinCash => "bch",
            Bitcoin => "btc",
            Ethereum => "eth",
            BitcoinTestnet => "btc-testnet",
            BitcoinCashTestnet => "bch-testnet",

            BitcoinGold => unreachable!(),
            Dash => unreachable!(),
            Litecoin => unreachable!(),
            EthereumGoerliTestnet => unreachable!(),
            LitecoinTestnet => unreachable!(),
            DashTestnet => unreachable!(),
            BitcoinSV => unreachable!(),
            BitcoinSVTestnet => unreachable!(),
            Doge => unreachable!(),
            Polkadot => unreachable!(),
            Solana => unreachable!(),
            SolanaTestnet => unreachable!(),
            Cardano => unreachable!(),
            Ripple => unreachable!(),
            RippleTestnet => unreachable!(),
            Stellar => unreachable!(),
            StellarTestnet => unreachable!(),
            ZCash => unreachable!(),
            ZCashTestnet => unreachable!(),
            Eos => unreachable!(),
            EosTestnet => unreachable!(),
            Avalanche => unreachable!(),
            Monero => unreachable!(),
            Kusama => unreachable!(),
            ECash => unreachable!(),
            Mixin => unreachable!(),
            Groestlcoin => unreachable!(),
            Algorand => unreachable!(),
            Celo => unreachable!(),
            Casper => unreachable!(),
            BinanceCoin => unreachable!(),
            AlgorandTestnet => unreachable!(),
            CeloTestnet => unreachable!(),
            CasperTestnet => unreachable!(),
            RSK => unreachable!(),
            Stacks => unreachable!(),
            RSKTestnet => unreachable!(),
            StacksTestnet => unreachable!(),
        }
    }
}

#[async_trait]
impl super::StaticSource for Blockchain {
    const ID: SourceId = SourceId::Blockchain;
    const SUPPORTED_CHAINS: &'static [ChainId] = &[
        Bitcoin,
        BitcoinCash,
        Ethereum,
        BitcoinTestnet,
        BitcoinCashTestnet,
    ];

    async fn check_updates(&self, recorder: &dyn ChainUpdateRecorder) {
        for &chain_id in Self::SUPPORTED_CHAINS {
            check_chain_update(
                recorder,
                &self.client,
                chain_id,
                Self::coin_symbol_for_chain(chain_id),
            )
            .await
        }
    }
}
