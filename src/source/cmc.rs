use crate::{get_now_ts, AppState, ChainName, ChainState, ChainStateRecorder, ChainStateUpdate};
use anyhow::{bail, Result};
use serde::Deserialize;

pub fn init(app_state: &mut AppState) {
    app_state.add_source(super::SOURCE_CMC);
    app_state.add_chain(super::CHAIN_BTC);
    app_state.add_chain(super::CHAIN_ETH);
    app_state.add_chain(super::CHAIN_LTC);
}

#[derive(Deserialize)]
struct BlocksBody {
    data: Vec<BlocksDataItem>,
}

#[derive(Deserialize)]
struct BlocksDataItem {
    hash: String,
    height: u64,
}

pub(crate) async fn get_chain_state(chain_api_symbol: &str) -> Result<ChainState> {
    let client = reqwest::Client::builder()
        .user_agent("curl/7.79.1")
        .build()?;
    let resp = client
        .get(format!(
            "https://blockchain.coinmarketcap.com/api/blocks?symbol={chain_api_symbol}&start=1&limit=1&quote=true"
        ))
        .send()
        .await?
        .error_for_status()?
        .json::<BlocksBody>()
        .await?;

    if let Some(item) = resp.data.get(0) {
        Ok(ChainState {
            ts: get_now_ts(),
            hash: item.hash.clone(),
            height: item.height,
        })
    } else {
        bail!("No blocks returned");
    }
}

async fn update_chain(recorder: &dyn ChainStateRecorder, chain: ChainName, chain_api_symbol: &str) {
    match get_chain_state(chain_api_symbol).await {
        Ok(state) => {
            recorder
                .update(ChainStateUpdate {
                    source: super::SOURCE_CMC.into(),
                    chain: chain.into(),
                    state,
                })
                .await
        }
        Err(e) => {
            tracing::warn!("Couldn't update CoinMarketCap {chain}: {e}");
        }
    }
}

pub(crate) async fn update(recorder: &dyn ChainStateRecorder) {
    update_chain(recorder, super::CHAIN_BTC.into(), "BTC").await;
    update_chain(recorder, super::CHAIN_ETH.into(), "ETH").await;
    update_chain(recorder, super::CHAIN_LTC.into(), "LTC").await;
}
