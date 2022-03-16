use super::{ChainId, ChainId::*, SourceId};
use crate::{get_now_ts, ChainState, ChainStateUpdate};
use anyhow::Result;
use axum::async_trait;
use serde::Deserialize;

#[derive(Deserialize)]
struct BlockLatestBody {
    id: String,
    height: u64,
}

pub(crate) async fn get_chain_state(
    client: &reqwest::Client,
    host: &str,
    chain_api_symbol: &str,
) -> Result<ChainState> {
    let resp = client
        .get(format!(
            "https://{host}/api/v2/{chain_api_symbol}/public/block/latest"
        ))
        .send()
        .await?
        .error_for_status()?
        .json::<BlockLatestBody>()
        .await?;

    Ok(ChainState {
        ts: get_now_ts(),
        hash: resp.id,
        height: resp.height,
    })
}

async fn get_chain_update(
    client: &reqwest::Client,
    chain: ChainId,
    host: &str,
    chain_api_symbol: &str,
) -> Option<ChainStateUpdate> {
    match get_chain_state(client, host, chain_api_symbol).await {
        Ok(state) => Some(ChainStateUpdate {
            source: SourceId::BitGo,
            chain: chain.into(),
            state,
        }),
        Err(e) => {
            let chain_name: &str = chain.into();
            tracing::warn!("Couldn't update BitGo {chain_name}: {e}");
            None
        }
    }
}

pub struct BitGo {
    client: reqwest::Client,
}

impl BitGo {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent("curl/7.79.1")
                .build()?,
        })
    }

    fn host_for_chain(chain: ChainId) -> &'static str {
        match chain {
            Bitcoin | BitcoinCash | Litecoin | Ethereum | Dash | Polkadot | BitcoinGold | BitcoinSV | Solana | Ripple | Stellar | ZCash | Eos
            | Avalanche | Algorand | Celo | Casper | RSK | Stacks => "bitgo.com",
            BitcoinTestnet | BitcoinCashTestnet | LitecoinTestnet | EthereumGoerliTestnet | DashTestnet | BitcoinSVTestnet | SolanaTestnet | RippleTestnet | StellarTestnet | EosTestnet | ZCashTestnet
            | AlgorandTestnet | CeloTestnet | CasperTestnet | RSKTestnet | StacksTestnet => "test.bitgo.com",
            Doge | Cardano | Monero | Kusama | ECash | Mixin | GroestlCoin | BinanceCoin => unreachable!(),
        }
    }

    fn coin_symbol_for_chain(chain: ChainId) -> &'static str {
        match chain {
            BitcoinCash => "bch",
            Bitcoin => "btc",
            BitcoinGold => "btg",
            Dash => "dash",
            Ethereum => "eth",
            Litecoin => "ltc",
            BitcoinCashTestnet => "tbch",
            BitcoinTestnet => "tbtc",
            EthereumGoerliTestnet => "teth",
            LitecoinTestnet => "tltc",
            DashTestnet => "tdash",
            BitcoinSV => "bsv",
            BitcoinSVTestnet => "tbsv",
            Doge => unreachable!(),
            Polkadot => unreachable!(),
            Solana => "sol",
            SolanaTestnet => "tsol",
            Cardano => unreachable!(),
            Ripple => "xrp",
            RippleTestnet => "txrp",
            Stellar => "xlm",
            StellarTestnet => "txlm",
            ZCash => "zec",
            ZCashTestnet => "tzec",
            Eos => "eos",
            EosTestnet => "teos",
            Avalanche => "avaxc",
            Monero => unreachable!(),
            Kusama => unreachable!(),
            ECash => unreachable!(),
            Mixin => unreachable!(),
            GroestlCoin => unreachable!(),
            Algorand => "algo",
            Celo => "celo",
            Casper => "cspr",
            BinanceCoin => unreachable!(),
            AlgorandTestnet => "talgo",
            CeloTestnet => "tcelo",
            CasperTestnet => "tcspr",
            RSK => "rbtc",
            Stacks => "stx",
            RSKTestnet => "trbtc",
            StacksTestnet => "tstx",
        }
    }
}

#[async_trait]
impl super::StaticSource for BitGo {
    const ID: SourceId = SourceId::BitGo;
    const SUPPORTED_CHAINS: &'static [ChainId] = &[
        Bitcoin, Litecoin, BitcoinCash, Dash, ZCash, BitcoinGold, BitcoinSV, Ethereum, Ripple, Stellar, Eos, Avalanche, Algorand, Celo, Casper, RSK, Stacks,
        BitcoinTestnet, LitecoinTestnet, BitcoinCashTestnet, DashTestnet, ZCashTestnet, BitcoinSVTestnet, EthereumGoerliTestnet, RippleTestnet, StellarTestnet, EosTestnet, AlgorandTestnet, CeloTestnet, CasperTestnet,
        RSKTestnet, StacksTestnet,
    ];

    async fn get_updates(&self) -> Vec<ChainStateUpdate> {
        let mut ret = vec![];
        for &chain_id in Self::SUPPORTED_CHAINS {
            if let Some(update) = get_chain_update(
                &self.client,
                chain_id,
                Self::host_for_chain(chain_id),
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
