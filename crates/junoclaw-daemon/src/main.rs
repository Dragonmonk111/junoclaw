use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use futures::{SinkExt, StreamExt};
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tracing::{info, warn};

use junoclaw_core::config::JunoClawConfig;
use junoclaw_core::types::{WsClientMessage, WsServerMessage};
use junoclaw_runtime::Runtime;

mod state;

use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config_path = JunoClawConfig::default_path();
    let config = if config_path.exists() {
        JunoClawConfig::load(&config_path)?
    } else {
        info!("No config found, using defaults. Run `junoclaw init` to create one.");
        JunoClawConfig::default()
    };

    let runtime = Runtime::new(&config).await?;
    let app_state = Arc::new(RwLock::new(AppState::new(config.clone(), runtime)));

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/api/health", get(health_handler))
        .route("/api/agents", get(list_agents_handler))
        .route("/api/config", get(config_handler))
        .layer(CorsLayer::permissive())
        .with_state(app_state.clone());

    let addr = SocketAddr::new(
        config.daemon.host.parse().unwrap_or([127, 0, 0, 1].into()),
        config.daemon.port,
    );

    info!("JunoClaw daemon starting on http://{}", addr);
    info!("Dashboard: http://{}:{}", config.daemon.host, config.daemon.port);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_handler() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "name": "junoclaw-daemon",
    }))
}

async fn config_handler(
    State(state): State<Arc<RwLock<AppState>>>,
) -> impl IntoResponse {
    let state = state.read().await;
    Json(serde_json::json!({
        "chain_enabled": state.config.chain.enabled,
        "chain_id": state.config.chain.chain_id,
        "wavs_enabled": state.config.wavs.enabled,
        "default_llm": state.config.llm.default_provider,
    }))
}

async fn list_agents_handler(
    State(state): State<Arc<RwLock<AppState>>>,
) -> impl IntoResponse {
    let state = state.read().await;
    let agents = state.runtime.list_agents().await;
    Json(agents)
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<RwLock<AppState>>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(socket: WebSocket, state: Arc<RwLock<AppState>>) {
    let (mut sender, mut receiver) = socket.split();

    let version = env!("CARGO_PKG_VERSION").to_string();
    let connected_msg = WsServerMessage::Connected { version };
    if let Ok(json) = serde_json::to_string(&connected_msg) {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
                let text_str: &str = &text;
                match serde_json::from_str::<WsClientMessage>(text_str) {
                    Ok(client_msg) => {
                        let (tx, mut rx) =
                            tokio::sync::mpsc::channel::<WsServerMessage>(64);

                        // Spawn the runtime handler so it can stream responses
                        let state_clone = state.clone();
                        tokio::spawn(async move {
                            let state_guard = state_clone.read().await;
                            state_guard.runtime.handle_message(client_msg, tx).await;
                        });

                        // Forward all responses from runtime to WS
                        while let Some(response) = rx.recv().await {
                            if let Ok(json) = serde_json::to_string(&response) {
                                if sender.send(Message::Text(json.into())).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Invalid WS message: {}", e);
                        let err = WsServerMessage::Error {
                            message: format!("Invalid message: {}", e),
                        };
                        if let Ok(json) = serde_json::to_string(&err) {
                            let _ = sender.send(Message::Text(json.into())).await;
                        }
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
}
