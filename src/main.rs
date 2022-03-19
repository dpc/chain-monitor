//! A simple web-app monitoring chain heights from various sources
use anyhow::Result;
use axum::{
    async_trait,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Extension, TypedHeader,
    },
    http::StatusCode,
    response::{Headers, Html, IntoResponse},
    routing::{get, get_service, IntoMakeService},
    Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::Serialize;
use source::{ChainId, Source, SourceId};
use std::{
    collections::{hash_map::Entry::*, HashMap},
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
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod opts;
mod source;
mod util;

use opts::Opts;

type SourceName = &'static str;
type ChainName = &'static str;

type ChainHeight = u64;
type BlockHash = String;

pub fn get_now_ts() -> u64 {
    u64::try_from(time::OffsetDateTime::now_utc().unix_timestamp()).expect("no negative timestamps")
}

#[derive(Serialize, Clone, Debug)]
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

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
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
pub struct WSChainStateUpdateTs {
    source: SourceId,
    chain: ChainId,
    first_seen_ts: u64,
    hash: BlockHash,
    height: ChainHeight,
}

// Our shared state
pub struct AppState {
    all_sources_names: Vec<SourceName>,
    all_sources_full_names: Vec<SourceName>,
    all_sources: Vec<SourceId>,
    all_chains_names: Vec<ChainName>,
    all_chains: Vec<ChainId>,
    all_chains_full_names: Vec<SourceName>,
    chain_states: Mutex<HashMap<(SourceId, ChainId), ChainStateTs>>,
    tx: broadcast::Sender<ChainStateUpdateTs>,
}

impl AppState {
    async fn get_all_chain_states(&self) -> Vec<ChainStateUpdateTs> {
        self.chain_states
            .lock()
            .await
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
        match self.all_sources.binary_search(&source) {
            Ok(_pos) => {}
            Err(pos) => {
                self.all_sources.insert(pos, source);
                self.all_sources_names.insert(pos, source.into());
                self.all_sources_full_names.insert(pos, source.full_name());
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

        match self.all_chains.binary_search(&chain) {
            Ok(_pos) => {}
            Err(pos) => {
                self.all_chains.insert(pos, chain);
                self.all_chains_names.insert(pos, chain.into());
                self.all_chains_full_names.insert(pos, chain.full_name());
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
            all_chains_names: Default::default(),
            all_chains_full_names: Default::default(),
            all_sources_names: Default::default(),
            all_sources_full_names: Default::default(),
            all_chains: Default::default(),
            all_sources: Default::default(),
            chain_states: Mutex::new(HashMap::new()),
            tx,
        }
    }
}

#[async_trait]
pub trait ChainUpdateRecorder: Sync {
    async fn update(&self, update: ChainStateUpdate);
}

#[async_trait]
impl ChainUpdateRecorder for AppState {
    async fn update(&self, update: ChainStateUpdate) {
        let (broadcast_update, state_ts) = {
            let state_ts = update.state.to_state_ts();
            match self
                .chain_states
                .lock()
                .await
                .entry((update.source, update.chain))
            {
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
}

type SharedAppState = Arc<AppState>;

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum WSMessage {
    #[serde(rename_all = "camelCase")]
    Init {
        sources: Vec<&'static str>,
        sources_full_name: Vec<&'static str>,
        chains: Vec<&'static str>,
        chains_full_name: Vec<&'static str>,
    },
    Update(WSChainStateUpdateTs),
}

fn setup_server(
    opts: &Opts,
    app_state: SharedAppState,
) -> axum::Server<hyper::server::conn::AddrIncoming, IntoMakeService<Router>> {
    let app = Router::new();

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

    let app = app
        // routes are matched from bottom to top, so we have to put `nest` at the
        // top since it matches all routes
        .route("/ws", get(ws_handler))
        // logging so we can see whats going on
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        )
        .layer(Extension(app_state));

    let addr = SocketAddr::from(([0, 0, 0, 0], opts.listen_port));
    let server = axum::Server::bind(&addr).serve(app.into_make_service());
    tracing::info!("listening on {}", server.local_addr());
    server
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
            sources: app_state.all_sources_names.clone(),
            sources_full_name: app_state.all_sources_full_names.clone(),
            chains: app_state.all_chains_names.clone(),
            chains_full_name: app_state.all_chains_full_names.clone(),
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
                .unwrap_or_else(|_| "chain_height_monitor=info,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let mut app_state = AppState::new();

    let source = source::get_source()?;
    app_state.add_chains(source.get_supported_chains());
    app_state.add_sources(source.get_supported_sources());

    let app_state = Arc::new(app_state);
    let server = setup_server(&opts, app_state.clone());
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
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}
