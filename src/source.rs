use crate::ChainUpdateRecorder;
use anyhow::Result;
use axum::async_trait;
use futures::future::join_all;
use serde::Serialize;
use std::collections::HashSet;
use strum::IntoStaticStr;
mod bitgo;
mod blockchain;
mod blockchair;
mod cmc;

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
    BlockchainInfo,
    Blockchair,
    CMC,
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
    GroestlCoin,
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
    ZCash,

    AlgorandTestnet,
    BitcoinCashTestnet,
    BitcoinSVTestnet,
    BitcoinTestnet,
    CasperTestnet,
    CeloTestnet,
    DashTestnet,
    EosTestnet,
    EthereumGoerliTestnet,
    LitecoinTestnet,
    RippleTestnet,
    RSKTestnet,
    SolanaTestnet,
    StacksTestnet,
    StellarTestnet,
    ZCashTestnet,
}

pub(crate) fn get_source() -> Result<Vec<Box<dyn Source>>> {
    Ok(vec![
        Box::new(bitgo::BitGo::new()?),
        Box::new(blockchain::Blockchain::new()?),
        Box::new(blockchair::Blockchair::new()?),
        Box::new(cmc::CoinMarketCap::new()?),
    ])
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
