use super::{ChainId, ChainId::*, SourceId};
use crate::{get_now_ts, ChainState, ChainStateUpdate};
use anyhow::Result;
use axum::async_trait;
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

async fn get_chain_update(
    client: &reqwest::Client,
    chain: ChainId,
    chain_api_symbol: &str,
) -> Option<ChainStateUpdate> {
    match get_chain_state(client, chain_api_symbol).await {
        Ok(state) => Some(ChainStateUpdate {
            source: SourceId::BlockchainInfo,
            chain: chain.into(),
            state,
        }),
        Err(e) => {
            let chain_name: &str = chain.into();
            tracing::warn!("Couldn't update Blockchain {chain_name}: {e}");
            None
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
            GroestlCoin => unreachable!(),
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
    const ID: SourceId = SourceId::BlockchainInfo;
    const SUPPORTED_CHAINS: &'static [ChainId] = &[
        Bitcoin,
        BitcoinCash,
        Ethereum,
        BitcoinTestnet,
        BitcoinCashTestnet,
    ];

    async fn get_updates(&self) -> Vec<ChainStateUpdate> {
        let mut ret = vec![];
        for &chain_id in Self::SUPPORTED_CHAINS {
            if let Some(update) = get_chain_update(
                &self.client,
                chain_id,
                Self::coin_symbol_for_chain(chain_id),
            )
            .await
            {
                ret.push(update);
            }
        }

        ret
    }
}
