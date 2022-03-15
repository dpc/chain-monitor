use crate::ChainStateRecorder;

mod bitgo;
mod blockchair;
mod cmc;

pub const SOURCE_BITGO: &'static str = "BitGo";
pub const SOURCE_BLOCKCHAIR: &'static str = "Blockchair";
pub const SOURCE_CMC: &'static str = "CoinMarketCap";

pub const CHAIN_BTC: &'static str = "Mainnet Bitcoin";
pub const CHAIN_TBTC: &'static str = "Testnet Bitcoin";
pub const CHAIN_ETH: &'static str = "Mainnet Ethereum";
pub const CHAIN_TETH: &'static str = "Testnet Ethereum";
pub const CHAIN_BCH: &'static str = "Mainnet Bitcoin Cash";
pub const CHAIN_TBCH: &'static str = "Testnet Bitcoin Cash";
pub const CHAIN_LTC: &'static str = "Mainnet Litecoin";
pub const _CHAIN_TLTC: &'static str = "Testnet Litecoin";

pub(crate) fn init_all(app_state: &mut crate::AppState) {
    self::bitgo::init(app_state);
    self::blockchair::init(app_state);
    self::cmc::init(app_state);
}

pub(crate) async fn update_all(app_state: &dyn ChainStateRecorder) {
    // TODO: create and use common `reqwest::Client`, so we can use connection pooling, set common user-agent
    // embed the source in to `ChainStateRecorder`, so that sources don't have to repeat it and can't mess it up
    tokio::join!(
        bitgo::update(app_state),
        blockchair::update(app_state),
        cmc::update(app_state)
    );
}
