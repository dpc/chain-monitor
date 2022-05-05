//! A simple web-app monitoring chain heights from various sources
use anyhow::Result;
use axum::{
    async_trait,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Extension, TypedHeader,
    },
    http::StatusCode,
    middleware,
    response::{Headers, Html, IntoResponse},
    routing::{get, get_service, IntoMakeService},
    Json, Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use metrics::gauge;
use serde::{Deserialize, Serialize};
use source::{ChainId, Source, SourceId};
use std::{
    cmp,
    collections::{hash_map::Entry::*, HashMap},
    future::ready,
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};
use tokio::{
    sync::{broadcast, Mutex},
    time::timeout,
};
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing::debug;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod opts;
mod prom;
mod source;
mod util;

use opts::Opts;

type ChainHeight = u64;
type BlockHash = String;

pub fn get_now_ts() -> u64 {
    u64::try_from(time::OffsetDateTime::now_utc().unix_timestamp()).expect("no negative timestamps")
}

#[derive(Serialize, Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainStateTs {
    first_seen_ts: u64,
    last_checked_ts: u64,
    #[serde(flatten)]
    state: ChainState,
}

impl ChainStateTs {
    fn update_by(&self, mut other: ChainStateTs) -> ChainStateTs {
        if self.state.height == other.state.height {
            other.first_seen_ts = self.first_seen_ts;
            other
        } else {
            other
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ChainState {
    hash: BlockHash,
    height: ChainHeight,
}

impl ChainState {
    fn to_state_ts(self) -> ChainStateTs {
        ChainStateTs {
            first_seen_ts: get_now_ts(),
            last_checked_ts: get_now_ts(),
            state: self,
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct ChainStateUpdate {
    source: SourceId,
    chain: ChainId,
    state: ChainState,
}
#[derive(Serialize, Clone, Debug)]
pub struct ChainStateUpdateTs {
    source: SourceId,
    chain: ChainId,
    #[serde(flatten)]
    state: ChainStateTs,
}

impl ChainStateUpdateTs {
    fn to_ws_update(self) -> WSChainStateUpdateTs {
        WSChainStateUpdateTs {
            first_seen_ts: self.state.first_seen_ts,
            hash: self.state.state.hash,
            height: self.state.state.height,
            source: self.source,
            chain: self.chain,
        }
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WSChainStateUpdateTs {
    source: SourceId,
    chain: ChainId,
    first_seen_ts: u64,
    hash: BlockHash,
    height: ChainHeight,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[derive(Debug)]
pub struct SourceInfo {
    id: SourceId,
    url: String,
    short_name: &'static str,
    full_name: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[derive(Debug)]
pub struct ChainInfo {
    id: ChainId,
    short_name: &'static str,
    full_name: &'static str,
    block_time_secs: u32,
}

#[derive(Default)]
pub struct ChainStates {
    states: HashMap<(SourceId, ChainId), ChainStateTs>,
    best_height: HashMap<ChainId, ChainHeight>,
}

impl ChainStates {
    fn to_best_states(&self) -> HashMap<&'static str, ChainStateTs> {
        self.best_height
            .iter()
            .map(|(best_height_chain, best_height)| {
                (
                    best_height_chain.ticker(),
                    self.states
                        .iter()
                        .filter(|((_, state_chain), state)| {
                            best_height_chain == state_chain && state.state.height == *best_height
                        })
                        .next()
                        .expect("must find something")
                        .1
                        .clone(),
                )
            })
            .collect()
    }
}

// Our shared state
pub struct AppState {
    sources: Vec<SourceInfo>,
    chains: Vec<ChainInfo>,
    chain_states: Mutex<ChainStates>,
    tx: broadcast::Sender<ChainStateUpdateTs>,
}

impl AppState {
    async fn get_all_chain_states(&self) -> Vec<ChainStateUpdateTs> {
        self.chain_states
            .lock()
            .await
            .states
            .iter()
            .map(|(k, v)| ChainStateUpdateTs {
                source: k.0.clone(),
                chain: k.1.clone(),
                state: v.clone(),
            })
            .collect()
    }

    fn subscribe_to_updates(&self) -> broadcast::Receiver<ChainStateUpdateTs> {
        self.tx.subscribe()
    }

    pub fn add_source(&mut self, source: SourceId) {
        match self
            .sources
            .binary_search_by_key(&source, |source_info| source_info.id)
        {
            Ok(_pos) => {}
            Err(pos) => {
                self.sources.insert(
                    pos,
                    SourceInfo {
                        id: source,
                        url: "tbd".into(),
                        short_name: source.short_name(),
                        full_name: source.full_name(),
                    },
                );
            }
        }
    }
    fn add_sources(&mut self, sources: std::collections::HashSet<SourceId>) {
        for source in sources {
            self.add_source(source);
        }
    }

    pub fn add_chain(&mut self, chain: ChainId) {
        let chain = chain.into();

        match self
            .chains
            .binary_search_by_key(&chain, |source_info| source_info.id)
        {
            Ok(_pos) => {}
            Err(pos) => {
                self.chains.insert(
                    pos,
                    ChainInfo {
                        id: chain,
                        block_time_secs: chain.block_time_secs(),
                        short_name: chain.short_name(),
                        full_name: chain.full_name(),
                    },
                );
            }
        }
    }
    fn add_chains(&mut self, chains: std::collections::HashSet<ChainId>) {
        for chain in chains {
            self.add_chain(chain);
        }
    }

    fn new() -> AppState {
        let (tx, _rx) = tokio::sync::broadcast::channel(1000);
        AppState {
            sources: Default::default(),
            chains: Default::default(),
            chain_states: Mutex::new(ChainStates::default()),
            tx,
        }
    }
}

#[async_trait]
pub trait ChainUpdateRecorder: Sync {
    async fn update(&self, update: ChainStateUpdate);
    async fn how_far_behind(&self, source: SourceId, chain: ChainId) -> ChainHeight;
}

#[async_trait]
impl ChainUpdateRecorder for AppState {
    async fn update(&self, update: ChainStateUpdate) {
        debug!(
            "{:?} {:?} update: {}",
            update.source, update.chain, update.state.height
        );

        gauge!(
            "chain_monitor_chain_height",
            update.state.height as f64,
            "source" => update.source.short_name().to_lowercase(),
            "chain" => update.chain.short_name().to_lowercase(),
            "ticker" => update.chain.ticker(),
            "network_type" => update.chain.network_type().to_string(),
            "source_full_name" => update.source.full_name(),
            "chain_full_name" => update.chain.full_name(),
        );

        let (broadcast_update, state_ts) = {
            let state_ts = update.state.to_state_ts();
            let mut chain_states = self.chain_states.lock().await;

            {
                let best_height = chain_states.best_height.entry(update.chain).or_insert(0);
                *best_height = cmp::max(*best_height, state_ts.state.height);
            }

            match chain_states.states.entry((update.source, update.chain)) {
                Occupied(mut e) => {
                    let old_state = e.get().clone();
                    let new_state = old_state.update_by(state_ts);
                    e.insert(new_state.clone());
                    (new_state.state != old_state.state, new_state)
                }
                Vacant(e) => {
                    e.insert(state_ts.clone());
                    (true, state_ts)
                }
            }
        };
        if broadcast_update {
            // we don't care if anyone is subscribed
            let _ = self.tx.send(ChainStateUpdateTs {
                source: update.source,
                chain: update.chain,
                state: state_ts,
            });
        }
    }
    async fn how_far_behind(&self, source: SourceId, chain: ChainId) -> ChainHeight {
        let chain_states = self.chain_states.lock().await;

        let cur_height = chain_states
            .states
            .get(&(source, chain))
            .map(|s| s.state.height)
            .unwrap_or(0);
        let cur_best_height = chain_states.best_height.get(&chain).cloned().unwrap_or(0);

        cur_best_height - cur_height
    }
}

type SharedAppState = Arc<AppState>;

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum WSMessage<'a> {
    #[serde(rename_all = "camelCase")]
    Init {
        sources: &'a [SourceInfo],
        chains: &'a [ChainInfo],
    },
    Update(WSChainStateUpdateTs),
}

fn setup_server(
    opts: &Opts,
    app_state: SharedAppState,
) -> Result<axum::Server<hyper::server::conn::AddrIncoming, IntoMakeService<Router>>> {
    let app = Router::new();

    let recorder_handle = prom::setup_metrics_recorder()?;

    // enable dynamic files if the feature is enabled
    let app = if opts.dynamic {
        app.fallback(
            get_service(ServeDir::new("assets").append_index_html_on_directories(true))
                .handle_error(|error: std::io::Error| async move {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Unhandled internal error: {}", error),
                    )
                }),
        )
    } else {
        app.route("/", get(index_html_handler))
            .route("/favicon.ico", get(favicon_ico_handler))
            .route("/style.css", get(style_css_handler))
            .route("/script.js", get(script_js_handler))
            .route("/sound1.mp3", get(sound1_mp3_handler))
    };

    let app = if opts.enable_prometheus {
        app.route("/metrics", get(move || ready(recorder_handle.render())))
    } else {
        app
    };

    let app = app.route("/state", get(get_state_handler));

    let app = app
        // routes are matched from bottom to top, so we have to put `nest` at the
        // top since it matches all routes
        .route("/ws", get(ws_handler))
        // logging so we can see whats going on
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        )
        .layer(Extension(app_state))
        .route_layer(middleware::from_fn(prom::track_metrics));

    let addr = SocketAddr::from(([0, 0, 0, 0], opts.listen_port));
    let server = axum::Server::bind(&addr).serve(app.into_make_service());
    tracing::info!("listening on {}", server.local_addr());
    Ok(server)
}

async fn index_html_handler() -> impl IntoResponse {
    (
        Headers([("Content-Type", "text/html")]),
        Html(include_str!("../assets/index.html")),
    )
}

async fn script_js_handler() -> impl IntoResponse {
    (
        Headers([("Content-Type", "application/javascript")]),
        include_str!("../assets/script.js"),
    )
}

async fn style_css_handler() -> impl IntoResponse {
    (
        Headers([("Content-Type", "text/css")]),
        include_str!("../assets/style.css"),
    )
}

async fn favicon_ico_handler() -> impl IntoResponse {
    (
        Headers([("Content-Type", "image/x-icon")]),
        include_bytes!("../assets/favicon.ico") as &'static [u8],
    )
}

async fn sound1_mp3_handler() -> impl IntoResponse {
    (
        Headers([("Content-Type", "audio/mpeg")]),
        include_bytes!("../assets/sound1.mp3") as &'static [u8],
    )
}

async fn get_state_handler(
    Extension(state): Extension<Arc<AppState>>,
) -> axum::extract::Json<HashMap<&'static str, ChainStateTs>> {
    Json(state.chain_states.lock().await.to_best_states())
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    Extension(state): Extension<Arc<AppState>>,
) -> impl IntoResponse {
    if let Some(TypedHeader(user_agent)) = user_agent {
        tracing::debug!("`{}` connected", user_agent.as_str());
    }

    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, app_state: SharedAppState) {
    if let Err(e) = handle_socket_try(socket, app_state).await {
        tracing::info!("Client disconnected: {e}");
    } else {
        tracing::info!("Client disconnected");
    }
}

async fn handle_socket_try(socket: WebSocket, app_state: SharedAppState) -> Result<()> {
    let (mut sender, _receiver) = socket.split();

    // subscribe early, so we don't miss anything
    let mut rx = app_state.subscribe_to_updates();

    // send all sources & chains info
    sender
        .send(Message::Text(serde_json::to_string(&WSMessage::Init {
            sources: &app_state.sources,
            chains: &app_state.chains,
        })?))
        .await?;

    // send all the existing updates
    for update in app_state.get_all_chain_states().await {
        sender
            .send(Message::Text(serde_json::to_string(&WSMessage::Update(
                update.to_ws_update(),
            ))?))
            .await?;
    }

    // keep sending new updates
    while let Ok(update) = rx.recv().await {
        sender
            .send(Message::Text(serde_json::to_string(&WSMessage::Update(
                update.to_ws_update(),
            ))?))
            .await?;
    }

    Ok(())
}

fn start_browser(url: String) {
    fn spawn(url: &str) -> Result<()> {
        let open_cmd = if cfg!(target_os = "windows") {
            "start"
        } else if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(target_os = "linux") {
            "xdg-open"
        } else {
            eprintln!("Unsupported platform. Please submit a PR!");
            return Ok(());
        };
        std::process::Command::new(open_cmd).arg(url).spawn()?;
        Ok(())
    }

    std::thread::spawn(move || {
        eprintln!("Opening browser pointing at {url}");
        let _ = spawn(&url);
    });
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = opts::from_args();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "chain_monitor=info,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let mut app_state = AppState::new();

    let source = source::get_source(&opts)?;
    app_state.add_chains(source.get_supported_chains());
    app_state.add_sources(source.get_supported_sources());

    let app_state = Arc::new(app_state);
    let server = setup_server(&opts, app_state.clone())?;
    let local_addr = server.local_addr();

    tokio::spawn({
        async move {
            server.await.unwrap();
        }
    });

    if !opts.daemon {
        start_browser(format!("http://{}", local_addr.to_string()));
    }

    loop {
        if let Err(e) = timeout(Duration::from_secs(30), source.check_updates(&*app_state)).await {
            tracing::warn!("Timeout waiting for updates: {e}");
        }
        tokio::time::sleep(Duration::from_secs(15)).await;
    }
}
