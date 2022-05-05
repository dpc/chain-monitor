use crate::{opts::Opts, ChainUpdateRecorder};
use anyhow::Result;
use axum::async_trait;
use futures::future::join_all;
use serde::Serialize;
use std::{
    cmp,
    collections::{HashMap, HashSet},
    fmt::Display,
};
use strum::IntoStaticStr;
use tokio::sync::Mutex;
use tracing::debug;

mod bitgo;
mod bitgov1;
mod blockchain;
mod blockchair;
mod blockcypher;
mod chainmonitor;
mod cmc;
mod mempoolspace;
mod other;

#[async_trait]
pub trait Source: Sync {
    fn get_supported_chains(&self) -> HashSet<ChainId>;
    fn get_supported_sources(&self) -> HashSet<SourceId>;

    async fn check_updates(&self, recorder: &dyn ChainUpdateRecorder);
}

/// Like `Source`, but doesn't do anything fancy,
/// so can use const fields
#[async_trait]
pub trait StaticSource: Sync {
    const ID: SourceId;
    const SUPPORTED_CHAINS: &'static [ChainId];

    async fn check_updates(&self, recorder: &dyn ChainUpdateRecorder);
}

// Any [`StaticSource`] is a [`Source`] too
#[async_trait]
impl<S> Source for S
where
    S: StaticSource,
{
    fn get_supported_chains(&self) -> HashSet<ChainId> {
        HashSet::from_iter(Self::SUPPORTED_CHAINS.iter().copied())
    }

    fn get_supported_sources(&self) -> HashSet<SourceId> {
        HashSet::from_iter(vec![Self::ID])
    }

    async fn check_updates(&self, recorder: &dyn ChainUpdateRecorder) {
        S::check_updates(&self, recorder).await
    }
}

#[derive(Debug, Clone, Copy, IntoStaticStr, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SourceId {
    BitGo,
    Blockchain,
    Blockchair,
    BlockCypher,
    CMC,
    MempoolSpace,
    BitGoV1,
    Other,
    ChainMonitor,
}

impl SourceId {
    pub fn full_name(self) -> &'static str {
        match self {
            SourceId::BitGo => "BitGo",
            SourceId::BitGoV1 => "BitGo (v1)",
            SourceId::Blockchain => "Blockchain.com",
            SourceId::Blockchair => "Blockchair",
            SourceId::BlockCypher => "BlockCypher",
            SourceId::MempoolSpace => "mempool.space",
            SourceId::CMC => "CoinMarketCap",
            SourceId::Other => "Other",
            SourceId::ChainMonitor => "ChainMonitor",
        }
    }
    pub fn short_name(self) -> &'static str {
        self.into()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum NetworkType {
    Mainnet,
    Testnet,
    Signet,
}

impl Display for NetworkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            NetworkType::Mainnet => "mainnet",
            NetworkType::Testnet => "testnet",
            NetworkType::Signet => "signet",
        })
    }
}

#[derive(Debug, Clone, Copy, IntoStaticStr, Hash, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChainId {
    Algorand,
    Avalanche,
    BinanceCoin,
    Bitcoin,
    BitcoinCash,
    BitcoinGold,
    BitcoinSV,
    Cardano,
    Casper,
    Celo,
    Dash,
    Doge,
    ECash,
    Eos,
    Ethereum,
    EthereumClassic,
    Groestlcoin,
    HederaHashgraph,
    Kusama,
    Litecoin,
    Mixin,
    Monero,
    Polkadot,
    Ripple,
    RSK,
    Solana,
    Stacks,
    Stellar,
    Tezos,
    ZCash,

    AlgorandTestnet,
    BitcoinCashTestnet,
    BitcoinSVTestnet,
    BitcoinTestnet,
    BitcoinSignet,
    CasperTestnet,
    CeloTestnet,
    DashTestnet,
    EosTestnet,
    EthereumGoerliTestnet,
    HederaHashgraphTestnet,
    LitecoinTestnet,
    RippleTestnet,
    RSKTestnet,
    SolanaTestnet,
    StacksTestnet,
    StellarTestnet,
    TezosTestnet,
    ZCashTestnet,
}

impl ChainId {
    pub fn block_time_secs(self) -> u32 {
        match self {
            ChainId::Bitcoin
            | ChainId::BitcoinCash
            | ChainId::BitcoinGold
            | ChainId::BitcoinSV
            | ChainId::BitcoinTestnet
            | ChainId::BitcoinCashTestnet
            | ChainId::BitcoinSVTestnet
            | ChainId::ECash
            | ChainId::Stacks
            | ChainId::StacksTestnet => 600,
            ChainId::Monero => 120,
            ChainId::ZCash | ChainId::ZCashTestnet => 75,
            ChainId::Litecoin | ChainId::LitecoinTestnet | ChainId::Dash | ChainId::DashTestnet => {
                150
            }
            ChainId::Ethereum | ChainId::EthereumClassic | ChainId::EthereumGoerliTestnet => 15,
            ChainId::Eos | Self::EosTestnet => 1, // actually 0.5, but we use integers, so whatever
            ChainId::Algorand | Self::AlgorandTestnet => 5, // actually 4.5
            ChainId::Tezos | Self::TezosTestnet => 30,
            // I'm kind of lazy RN, so default to some sanity value for now
            _ => 120,
        }
    }
    pub fn full_name(self) -> &'static str {
        match self {
            ChainId::Algorand => "Algorand",
            ChainId::Avalanche => "Avalanche",
            ChainId::BinanceCoin => "Binance Coin",
            ChainId::Bitcoin => "Bitcoin",
            ChainId::BitcoinCash => "Bitcoin Cash",
            ChainId::BitcoinGold => "Bitcoin Gold",
            ChainId::BitcoinSV => "Bitcoin SV",
            ChainId::Cardano => "Cardano",
            ChainId::Casper => "Casper",
            ChainId::Celo => "Celo",
            ChainId::Dash => "Dash",
            ChainId::Doge => "Doge",
            ChainId::ECash => "ECash",
            ChainId::Eos => "Eos",
            ChainId::Ethereum => "Ethereum",
            ChainId::EthereumClassic => "Ethereum Classic",
            ChainId::Groestlcoin => "Groestlcoin",
            ChainId::HederaHashgraph => "Hedera Hashgraph",
            ChainId::Kusama => "Kusama",
            ChainId::Litecoin => "Litecoin",
            ChainId::Mixin => "Mixin",
            ChainId::Monero => "Monero",
            ChainId::Polkadot => "Polkadot",
            ChainId::Ripple => "Ripple",
            ChainId::RSK => "RSK",
            ChainId::Solana => "Solana",
            ChainId::Stacks => "Stacks",
            ChainId::Stellar => "Stellar",
            ChainId::Tezos => "Tezos",
            ChainId::ZCash => "ZCash",
            ChainId::AlgorandTestnet => "Algorand Testnet",
            ChainId::BitcoinCashTestnet => "Bitcoin Cash Testnet",
            ChainId::BitcoinSVTestnet => "Bitcoin SV Testnet",
            ChainId::BitcoinTestnet => "Bitcoin Testnet",
            ChainId::CasperTestnet => "Casper Testnet",
            ChainId::CeloTestnet => "Celo Testnet",
            ChainId::DashTestnet => "Dash Testnet",
            ChainId::EosTestnet => "Eos Testnet",
            ChainId::EthereumGoerliTestnet => "Ethereum Testnet (Goerli) ",
            ChainId::HederaHashgraphTestnet => "Hedera Hashgraph Testnet",
            ChainId::LitecoinTestnet => "Litecoin Testnet",
            ChainId::RippleTestnet => "Ripple Testnet",
            ChainId::RSKTestnet => "RSK Testnet",
            ChainId::SolanaTestnet => "Solana Testnet",
            ChainId::StacksTestnet => "Stacks Testnet",
            ChainId::StellarTestnet => "Stellar Testnet",
            ChainId::TezosTestnet => "Tezos Testnet",
            ChainId::ZCashTestnet => "ZCash Testnet",
            ChainId::BitcoinSignet => "Bitcoin Signet",
        }
    }
    pub fn short_name(self) -> &'static str {
        self.into()
    }

    pub fn from_ticker(ticker: &str) -> Option<Self> {
        Some(match ticker {
            "algo" => ChainId::Algorand,
            "avax" => ChainId::Avalanche,
            "bnb" => ChainId::BinanceCoin,
            "btc" => ChainId::Bitcoin,
            "bch" => ChainId::BitcoinCash,
            "btg" => ChainId::BitcoinGold,
            "bsv" => ChainId::BitcoinSV,
            "ada" => ChainId::Cardano,
            "cspr" => ChainId::Casper,
            "celo" => ChainId::Celo,
            "dash" => ChainId::Dash,
            "doge" => ChainId::Doge,
            "xec" => ChainId::ECash,
            "eos" => ChainId::Eos,
            "eth" => ChainId::Ethereum,
            "etc" => ChainId::EthereumClassic,
            "grs" => ChainId::Groestlcoin,
            "hbar" => ChainId::HederaHashgraph,
            "ksm" => ChainId::Kusama,
            "ltc" => ChainId::Litecoin,
            "xin" => ChainId::Mixin,
            "mnr" => ChainId::Monero,
            "dot" => ChainId::Polkadot,
            "xrp" => ChainId::Ripple,
            "rbtc" => ChainId::RSK,
            "sol" => ChainId::Solana,
            "stx" => ChainId::Stacks,
            "xlm" => ChainId::Stellar,
            "xtz" => ChainId::Tezos,
            "zec" => ChainId::ZCash,
            "algo-testnet" => ChainId::AlgorandTestnet,
            "bch-testnet" => ChainId::BitcoinCashTestnet,
            "bsv-testnet" => ChainId::BitcoinSVTestnet,
            "btc-testnet" => ChainId::BitcoinTestnet,
            "cspr-testnet" => ChainId::CasperTestnet,
            "celo-testnet" => ChainId::CeloTestnet,
            "dash-testnet" => ChainId::DashTestnet,
            "eos-testnet" => ChainId::EosTestnet,
            "eth-testnet" => ChainId::EthereumGoerliTestnet,
            "thbar" => ChainId::HederaHashgraphTestnet,
            "ltc-testnet" => ChainId::LitecoinTestnet,
            "xrp-testnet" => ChainId::RippleTestnet,
            "rbtc-testnet" => ChainId::RSKTestnet,
            "sol-testnet" => ChainId::SolanaTestnet,
            "stx-testnet" => ChainId::StacksTestnet,
            "xlm-testnet" => ChainId::StellarTestnet,
            "xtz-testnet" => ChainId::TezosTestnet,
            "zec-testnet" => ChainId::ZCashTestnet,
            "btc-signet" => ChainId::BitcoinSignet,
            _ => return None,
        })
    }

    pub fn ticker(self) -> &'static str {
        match self {
            ChainId::Algorand => "algo",
            ChainId::Avalanche => "avax",
            ChainId::BinanceCoin => "bnb",
            ChainId::Bitcoin => "btc",
            ChainId::BitcoinCash => "bch",
            ChainId::BitcoinGold => "btg",
            ChainId::BitcoinSV => "bsv",
            ChainId::Cardano => "ada",
            ChainId::Casper => "cspr",
            ChainId::Celo => "celo",
            ChainId::Dash => "dash",
            ChainId::Doge => "doge",
            ChainId::ECash => "xec",
            ChainId::Eos => "eos",
            ChainId::Ethereum => "eth",
            ChainId::EthereumClassic => "etc",
            ChainId::Groestlcoin => "grs",
            ChainId::HederaHashgraph => "hbar",
            ChainId::Kusama => "ksm",
            ChainId::Litecoin => "ltc",
            ChainId::Mixin => "xin",
            ChainId::Monero => "mnr",
            ChainId::Polkadot => "dot",
            ChainId::Ripple => "xrp",
            ChainId::RSK => "rbtc",
            ChainId::Solana => "sol",
            ChainId::Stacks => "stx",
            ChainId::Stellar => "xlm",
            ChainId::Tezos => "xtz",
            ChainId::ZCash => "zec",
            ChainId::AlgorandTestnet => "algo-testnet",
            ChainId::BitcoinCashTestnet => "bch-testnet",
            ChainId::BitcoinSVTestnet => "bsv-testnet",
            ChainId::BitcoinTestnet => "btc-testnet",
            ChainId::CasperTestnet => "cspr-testnet",
            ChainId::CeloTestnet => "celo-testnet",
            ChainId::DashTestnet => "dash-testnet",
            ChainId::EosTestnet => "eos-testnet",
            ChainId::EthereumGoerliTestnet => "eth-testnet",
            ChainId::HederaHashgraphTestnet => "thbar",
            ChainId::LitecoinTestnet => "ltc-testnet",
            ChainId::RippleTestnet => "xrp-testnet",
            ChainId::RSKTestnet => "rbtc-testnet",
            ChainId::SolanaTestnet => "sol-testnet",
            ChainId::StacksTestnet => "stx-testnet",
            ChainId::StellarTestnet => "xlm-testnet",
            ChainId::TezosTestnet => "xtz-testnet",
            ChainId::ZCashTestnet => "zec-testnet",
            ChainId::BitcoinSignet => "btc-signet",
        }
    }

    pub fn network_type(self) -> NetworkType {
        match self {
            ChainId::Algorand => NetworkType::Mainnet,
            ChainId::Avalanche => NetworkType::Mainnet,
            ChainId::BinanceCoin => NetworkType::Mainnet,
            ChainId::Bitcoin => NetworkType::Mainnet,
            ChainId::BitcoinCash => NetworkType::Mainnet,
            ChainId::BitcoinGold => NetworkType::Mainnet,
            ChainId::BitcoinSV => NetworkType::Mainnet,
            ChainId::Cardano => NetworkType::Mainnet,
            ChainId::Casper => NetworkType::Mainnet,
            ChainId::Celo => NetworkType::Mainnet,
            ChainId::Dash => NetworkType::Mainnet,
            ChainId::Doge => NetworkType::Mainnet,
            ChainId::ECash => NetworkType::Mainnet,
            ChainId::Eos => NetworkType::Mainnet,
            ChainId::Ethereum => NetworkType::Mainnet,
            ChainId::EthereumClassic => NetworkType::Mainnet,
            ChainId::Groestlcoin => NetworkType::Mainnet,
            ChainId::HederaHashgraph => NetworkType::Mainnet,
            ChainId::Kusama => NetworkType::Mainnet,
            ChainId::Litecoin => NetworkType::Mainnet,
            ChainId::Mixin => NetworkType::Mainnet,
            ChainId::Monero => NetworkType::Mainnet,
            ChainId::Polkadot => NetworkType::Mainnet,
            ChainId::Ripple => NetworkType::Mainnet,
            ChainId::RSK => NetworkType::Mainnet,
            ChainId::Solana => NetworkType::Mainnet,
            ChainId::Stacks => NetworkType::Mainnet,
            ChainId::Stellar => NetworkType::Mainnet,
            ChainId::Tezos => NetworkType::Mainnet,
            ChainId::ZCash => NetworkType::Mainnet,
            ChainId::AlgorandTestnet => NetworkType::Testnet,
            ChainId::BitcoinCashTestnet => NetworkType::Testnet,
            ChainId::BitcoinSVTestnet => NetworkType::Testnet,
            ChainId::BitcoinTestnet => NetworkType::Testnet,
            ChainId::CasperTestnet => NetworkType::Testnet,
            ChainId::CeloTestnet => NetworkType::Testnet,
            ChainId::DashTestnet => NetworkType::Testnet,
            ChainId::EosTestnet => NetworkType::Testnet,
            ChainId::EthereumGoerliTestnet => NetworkType::Testnet,
            ChainId::HederaHashgraphTestnet => NetworkType::Testnet,
            ChainId::LitecoinTestnet => NetworkType::Testnet,
            ChainId::RippleTestnet => NetworkType::Testnet,
            ChainId::RSKTestnet => NetworkType::Testnet,
            ChainId::SolanaTestnet => NetworkType::Testnet,
            ChainId::StacksTestnet => NetworkType::Testnet,
            ChainId::StellarTestnet => NetworkType::Testnet,
            ChainId::TezosTestnet => NetworkType::Testnet,
            ChainId::ZCashTestnet => NetworkType::Testnet,
            ChainId::BitcoinSignet => NetworkType::Signet,
        }
    }
}

pub(crate) fn get_source(opts: &Opts) -> Result<Vec<Box<dyn Source>>> {
    let mut sources = vec![
        Box::new(bitgo::BitGo::new()?) as Box<dyn Source>,
        Box::new(bitgov1::BitGoV1::new()?),
        Box::new(blockchain::Blockchain::new()?),
        Box::new(blockchair::Blockchair::new()?),
        Box::new(blockcypher::BlockCypher::new()?),
        Box::new(mempoolspace::MempoolSpace::new()?),
        Box::new(cmc::CoinMarketCap::new()?),
        Box::new(other::Other::new()?),
    ];

    for mirror in &opts.mirror {
        sources.push(Box::new(chainmonitor::ChainMonitor::new(mirror.clone())?) as Box<dyn Source>)
    }
    Ok(sources)
}

#[async_trait]
impl Source for Vec<Box<dyn Source>> {
    fn get_supported_chains(&self) -> HashSet<ChainId> {
        self.iter().fold(HashSet::new(), |set, source| {
            set.union(&source.get_supported_chains()).cloned().collect()
        })
    }

    fn get_supported_sources(&self) -> HashSet<SourceId> {
        self.iter().fold(HashSet::new(), |set, source| {
            set.union(&source.get_supported_sources())
                .cloned()
                .collect()
        })
    }

    async fn check_updates(&self, recorder: &dyn ChainUpdateRecorder) {
        join_all(self.iter().map(|source| source.check_updates(recorder))).await;
    }
}

struct UpdateRateLimiter {
    source: SourceId,
    last_checked: Mutex<HashMap<ChainId, u64>>,
    enable_periodic_check: bool,
}

impl UpdateRateLimiter {
    fn new(source: SourceId) -> Self {
        Self {
            source,
            last_checked: Mutex::new(HashMap::default()),
            enable_periodic_check: true,
        }
    }

    pub fn disable_periodic_check(self) -> Self {
        Self {
            enable_periodic_check: false,
            ..self
        }
    }

    async fn should_check(
        &self,
        chain: ChainId,
        update_recorder: &dyn ChainUpdateRecorder,
    ) -> bool {
        let now = super::get_now_ts();
        let mut last_checked = self.last_checked.lock().await;

        let since_last_check_secs = now - *last_checked.entry(chain).or_insert(0);
        let recheck_threashold_secs = cmp::max(u64::from(chain.block_time_secs()) / 2, 45);
        let how_far_behind = update_recorder.how_far_behind(self.source, chain).await;

        let is_behind = if how_far_behind > 0 {
            debug!(
                "{:?} {:?} is {} behind; updating",
                self.source, chain, how_far_behind
            );
            true
        } else {
            false
        };

        let is_stale =
            if (since_last_check_secs > recheck_threashold_secs) && self.enable_periodic_check {
                debug!(
                    "{:?} {:?} is {}s since last updated; updating",
                    self.source, chain, since_last_check_secs
                );
                true
            } else {
                false
            };

        if is_behind || is_stale {
            last_checked.insert(chain, now);
            true
        } else {
            false
        }
    }
}
