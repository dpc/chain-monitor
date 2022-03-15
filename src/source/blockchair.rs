use crate::{get_now_ts, AppState, ChainState, ChainStateRecorder, ChainStateUpdate};
use anyhow::Result;
use serde::Deserialize;

pub fn init(app_state: &mut AppState) {
    app_state.add_source(super::SOURCE_BLOCKCHAIR);
    app_state.add_chain(super::CHAIN_BTC);
    app_state.add_chain(super::CHAIN_ETH);
    app_state.add_chain(super::CHAIN_BCH);
}

// TODO: find a nicer way; whoever made this API scheme, really love nesting shit and the word "data"
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
struct HomepageEnBody {
    data: HomepageEnData,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
struct HomepageEnData {
    stats: HomepageEnStats,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
struct HomepageEnStats {
    data: HomepageEnDataInner,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct HomepageEnDataInner {
    bitcoin: HomepageEnCoin,
    bitcoin_cash: HomepageEnCoin,
    ethereum: HomepageEnCoin,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
struct HomepageEnCoin {
    data: HomepageEnCoinData,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
struct HomepageEnCoinData {
    best_block_height: u64,
    best_block_hash: String,
}

async fn get_homepage_en() -> Result<HomepageEnBody> {
    Ok(
        reqwest::get("https://api.blockchair.com/internal/homepage/en")
            .await?
            .json::<HomepageEnBody>()
            .await?,
    )
}

async fn update_chain(recorder: &dyn ChainStateRecorder) {
    let ts = get_now_ts();
    match get_homepage_en().await {
        Ok(state) => {
            recorder
                .update(ChainStateUpdate {
                    source: super::SOURCE_BLOCKCHAIR.into(),
                    chain: super::CHAIN_BTC.into(),
                    state: ChainState {
                        ts,
                        hash: state.data.stats.data.bitcoin.data.best_block_hash,
                        height: state.data.stats.data.bitcoin.data.best_block_height,
                    },
                })
                .await;
            recorder
                .update(ChainStateUpdate {
                    source: super::SOURCE_BLOCKCHAIR.into(),
                    chain: super::CHAIN_BCH.into(),
                    state: ChainState {
                        ts,
                        hash: state.data.stats.data.bitcoin_cash.data.best_block_hash,
                        height: state.data.stats.data.bitcoin_cash.data.best_block_height,
                    },
                })
                .await;
            recorder
                .update(ChainStateUpdate {
                    source: super::SOURCE_BLOCKCHAIR.into(),
                    chain: super::CHAIN_ETH.into(),
                    state: ChainState {
                        ts,
                        hash: state.data.stats.data.ethereum.data.best_block_hash,
                        height: state.data.stats.data.ethereum.data.best_block_height,
                    },
                })
                .await;
        }
        Err(e) => {
            tracing::warn!("Couldn't update Blockchair: {e}");
        }
    }
}

pub(crate) async fn update(recorder: &dyn ChainStateRecorder) {
    update_chain(recorder).await;
}
