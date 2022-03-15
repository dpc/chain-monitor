//! A simple web-app monitoring chain heights from various sources
use anyhow::Result;
use axum::{
    async_trait,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Extension, TypedHeader,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, get_service, IntoMakeService},
    Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::Serialize;
use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};
use tokio::sync::{broadcast, Mutex};
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod opts;
mod source;

use opts::Opts;

type SourceName = String;
type ChainName = String;

type ChainHeight = u64;
type BlockHash = String;

pub fn get_now_ts() -> u64 {
    u64::try_from(time::OffsetDateTime::now_utc().unix_timestamp()).expect("no negativ timestamps")
}

#[derive(Serialize, Clone, Debug)]
struct ChainState {
    ts: u64,
    hash: BlockHash,
    height: ChainHeight,
}

#[derive(Serialize, Clone, Debug)]
struct ChainStateUpdate {
    source: SourceName,
    chain: ChainName,
    state: ChainState,
}

// Our shared state
pub struct AppState {
    all_sources: Vec<SourceName>,
    all_chains: Vec<ChainName>,
    chain_states: Mutex<HashMap<(SourceName, ChainName), ChainState>>,
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

    pub fn add_source(&mut self, source: impl Into<SourceName>) {
        let source = source.into();

        match self.all_sources.binary_search(&source) {
            Ok(_pos) => {}
            Err(pos) => self.all_sources.insert(pos, source),
        }
    }

    pub fn add_chain(&mut self, chain: impl Into<SourceName>) {
        let chain = chain.into();

        match self.all_chains.binary_search(&chain) {
            Ok(_pos) => {}
            Err(pos) => self.all_chains.insert(pos, chain),
        }
    }

    fn new() -> AppState {
        let (tx, _rx) = tokio::sync::broadcast::channel(1000);
        AppState {
            all_chains: Default::default(),
            all_sources: Default::default(),
            chain_states: Mutex::new(HashMap::new()),
            tx,
        }

        /*
        WSMessage::Update(ChainStateUpdate {
                                source: "x".into(),
                                chain: "c1".into(),
                                state: ChainState {
                                    ts: time::OffsetDateTime::now_utc().unix_timestamp(),
                                    hash: "234".into(),
                                    height: 3,
                                },
                            }         */
    }
}
#[async_trait]
trait ChainStateRecorder: Sync {
    async fn update(&self, update: ChainStateUpdate);
}

#[async_trait]
impl ChainStateRecorder for AppState {
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
        sources: Vec<String>,
        chains: Vec<String>,
    },
    Update(ChainStateUpdate),
}

fn setup_server(
    opts: &Opts,
    app_state: SharedAppState,
) -> axum::Server<hyper::server::conn::AddrIncoming, IntoMakeService<Router>> {
    // build our application with some routes
    let app = Router::new()
        .fallback(
            get_service(ServeDir::new("assets").append_index_html_on_directories(true))
                .handle_error(|error: std::io::Error| async move {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Unhandled internal error: {}", error),
                    )
                }),
        )
        // routes are matched from bottom to top, so we have to put `nest` at the
        // top since it matches all routes
        .route("/ws", get(ws_handler))
        // logging so we can see whats going on
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        )
        .layer(Extension(app_state));

    // run it with hyper
    let addr = SocketAddr::from(([0, 0, 0, 0], opts.listen_port));
    let server = axum::Server::bind(&addr).serve(app.into_make_service());
    tracing::info!("listening on {}", server.local_addr());
    server
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
            sources: app_state.all_sources.clone(),
            chains: app_state.all_chains.clone(),
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

#[tokio::main]
async fn main() {
    let opts = opts::from_args();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "chain_height_monitor=info,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let mut app_state = AppState::new();

    source::init_all(&mut app_state);

    let app_state = Arc::new(app_state);

    tokio::spawn({
        let app_state = app_state.clone();
        async move {
            loop {
                source::update_all(&*app_state as &dyn ChainStateRecorder).await;
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        }
    });

    setup_server(&opts, app_state.clone()).await.unwrap();
}
