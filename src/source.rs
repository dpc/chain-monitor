use crate::ChainStateRecorder;

mod bitgo;

pub const SOURCE_BITGO: &'static str = "BitGo";
pub const CHAIN_BTC: &'static str = "Mainnet Bitcoin";
pub const CHAIN_TBTC: &'static str = "Testnet Bitcoin";
pub const CHAIN_ETH: &'static str = "Mainnet Ethereum";
pub const CHAIN_TETH: &'static str = "Testnet Ethereum";

pub(crate) fn init_all(app_state: &mut crate::AppState) {
    self::bitgo::init(app_state);
}

pub(crate) async fn update_all(app_state: &dyn ChainStateRecorder) {
    futures::future::join_all([bitgo::update(app_state)]).await;
}
