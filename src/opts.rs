use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[clap(
    name = "chain-height-monitor",
    about = "(block-)Chain Height Monitor Utility/Server"
)]
pub struct Opts {
    /// Port to listen on
    #[clap(long = "listen", short = 'l', default_value = "0")]
    pub listen_port: u16,
}

pub fn from_args() -> Opts {
    Opts::parse()
}