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
use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};
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
    u64::try_from(time::OffsetDateTime::now_utc().unix_timestamp()).expect("no negativ timestamps")
}

#[derive(Serialize, Clone, Debug)]
pub struct ChainState {
    ts: u64,
    hash: BlockHash,
    height: ChainHeight,
}

#[derive(Serialize, Clone, Debug)]
pub struct ChainStateUpdate {
    source: SourceId,
    chain: ChainId,
    state: ChainState,
}

// Our shared state
pub struct AppState {
    all_sources_names: Vec<SourceName>,
    all_sources: Vec<SourceId>,
    all_chains_names: Vec<ChainName>,
    all_chains: Vec<ChainId>,
    chain_states: Mutex<HashMap<(SourceId, ChainId), ChainState>>,
    tx: broadcast::Sender<ChainStateUpdate>,
}

impl AppState {
    async fn get_all_chain_states(&self) -> Vec<ChainStateUpdate> {
        self.chain_states
            .lock()
            .await
            .iter()
            .map(|(k, v)| ChainStateUpdate {
                source: k.0.clone(),
                chain: k.1.clone(),
                state: v.clone(),
            })
            .collect()
    }

    fn subscribe_to_updates(&self) -> broadcast::Receiver<ChainStateUpdate> {
        self.tx.subscribe()
    }

    pub fn add_source(&mut self, source: SourceId) {
        match self.all_sources.binary_search(&source) {
            Ok(_pos) => {}
            Err(pos) => {
                self.all_sources.insert(pos, source);
                self.all_sources_names.insert(pos, source.into());
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
            all_sources_names: Default::default(),
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
        {
            let update = update.clone();
            self.chain_states
                .lock()
                .await
                .insert((update.source, update.chain), update.state);
        }
        // we don't care if anyone is subscribed
        let _ = self.tx.send(update);
    }
}

type SharedAppState = Arc<AppState>;

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum WSMessage {
    Init {
        sources: Vec<&'static str>,
        chains: Vec<&'static str>,
    },
    Update(ChainStateUpdate),
}

fn setup_server(
    opts: &Opts,
    app_state: SharedAppState,
) -> axum::Server<hyper::server::conn::AddrIncoming, IntoMakeService<Router>> {
    let app = Router::new();

    // enable dynamic files if the feature is enabled
    let app = if cfg!(dynamic) {
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

async fn favicon_ico_handler() -> &'static [u8] {
    include_bytes!("../assets/favicon.ico")
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
            chains: app_state.all_chains_names.clone(),
        })?))
        .await?;

    // send all the existing updates
    for update in app_state.get_all_chain_states().await {
        sender
            .send(Message::Text(serde_json::to_string(&WSMessage::Update(
                update,
            ))?))
            .await?;
    }

    // keep sending new updates
    while let Ok(update) = rx.recv().await {
        sender
            .send(Message::Text(serde_json::to_string(&WSMessage::Update(
                update,
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
