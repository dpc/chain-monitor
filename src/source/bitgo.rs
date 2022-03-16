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
            Btc | Bch | Ltc | Eth | Dash | Dot | Btg | Bsv | Sol | Xrp | Xlm | Zec | Eos
            | Avaxc => "bitgo.com",
            TBtc | TBch | TLtc | TEthGoerli | TDash | TBsv | TSol | TXrp | TXlm | TEos | TZec => {
                "test.bitgo.com"
            }
            Doge | Cardano | Xmr | Kusama | ECash | Mixin | Groestlcoin => unreachable!(),
        }
    }

    fn coin_symbol_for_cain(chain: ChainId) -> &'static str {
        match chain {
            Bch => "bch",
            Btc => "btc",
            Btg => "btg",
            Dash => "dash",
            Eth => "eth",
            Ltc => "ltc",
            TBch => "tbch",
            TBtc => "tbtc",
            TEthGoerli => "teth",
            TLtc => "tltc",
            TDash => "tdash",
            Bsv => "bsv",
            TBsv => "tbsv",
            Doge => unreachable!(),
            Dot => unreachable!(),
            Sol => "sol",
            TSol => "tsol",
            Cardano => unreachable!(),
            Xrp => "xrp",
            TXrp => "txrp",
            Xlm => "xlm",
            TXlm => "txlm",
            Zec => "zec",
            TZec => "tzec",
            Eos => "eos",
            TEos => "teos",
            Avaxc => "avaxc",
            Xmr => unreachable!(),
            Kusama => unreachable!(),
            ECash => unreachable!(),
            Mixin => unreachable!(),
            Groestlcoin => unreachable!(),
        }
    }
}

#[async_trait]
impl super::StaticSource for BitGo {
    const ID: SourceId = SourceId::BitGo;
    const SUPPORTED_CHAINS: &'static [ChainId] = &[
        Btc, Ltc, Bch, Dash, Zec, Btg, Bsv, Eth, Xrp, Xlm, Eos, Avaxc, TBtc, TLtc, TBch, TDash,
        TZec, TBsv, TEthGoerli, TXrp, TXlm, TEos,
    ];

    async fn get_updates(&self) -> Vec<ChainStateUpdate> {
        let mut ret = vec![];
        for &chain_id in Self::SUPPORTED_CHAINS {
            if let Some(update) = get_chain_update(
                &self.client,
                chain_id,
                Self::host_for_chain(chain_id),
                Self::coin_symbol_for_cain(chain_id),
            )
            .await
            {
                ret.push(update);
            }
        }

        ret
    }
}
