use crate::{get_now_ts, AppState, ChainName, ChainState, ChainStateRecorder, ChainStateUpdate};
use anyhow::Result;
use serde::Deserialize;

pub fn init(app_state: &mut AppState) {
    app_state.add_source(super::SOURCE_BITGO);
    app_state.add_chain(super::CHAIN_BTC);
    app_state.add_chain(super::CHAIN_TBTC);
    app_state.add_chain(super::CHAIN_ETH);
    app_state.add_chain(super::CHAIN_TETH);
}

#[derive(Deserialize)]
struct BlockLatestBody {
    id: String,
    height: u64,
}

pub(crate) async fn get_chain_state(host: &str, chain_api_symbol: &str) -> Result<ChainState> {
    let resp = reqwest::get(format!(
        "https://{host}/api/v2/{chain_api_symbol}/public/block/latest"
    ))
    .await?
    .json::<BlockLatestBody>()
    .await?;

    Ok(ChainState {
        ts: get_now_ts(),
        hash: resp.id,
        height: resp.height,
    })
}

async fn update_chain(
    recorder: &dyn ChainStateRecorder,
    chain: ChainName,
    host: &str,
    chain_api_symbol: &str,
) {
    match get_chain_state(host, chain_api_symbol).await {
        Ok(state) => {
            recorder
                .update(ChainStateUpdate {
                    source: super::SOURCE_BITGO.into(),
                    chain: chain.into(),
                    state,
                })
                .await
        }
        Err(e) => {
            tracing::warn!("Couldn't update BitGo {chain}: {e}");
        }
    }
}

pub(crate) async fn update(recorder: &dyn ChainStateRecorder) {
    update_chain(recorder, super::CHAIN_BTC.into(), "bitgo.com", "btc").await;
    update_chain(recorder, super::CHAIN_TBTC.into(), "test.bitgo.com", "tbtc").await;

    update_chain(recorder, super::CHAIN_ETH.into(), "bitgo.com", "eth").await;
    update_chain(recorder, super::CHAIN_TETH.into(), "test.bitgo.com", "teth").await;

    update_chain(recorder, super::CHAIN_BCH.into(), "bitgo.com", "bch").await;
    update_chain(recorder, super::CHAIN_TBCH.into(), "test.bitgo.com", "tbch").await;
}
