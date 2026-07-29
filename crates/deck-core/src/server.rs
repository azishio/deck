//! Local Axum server (design doc 14).

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use axum::Router;
use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use camino::{Utf8Path, Utf8PathBuf};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::broadcast;

use crate::assets::{self, Page};
use crate::config::Overrides;
use crate::error::{Error, Result};
use crate::manifest::Manifest;
use crate::project::Project;
use crate::render::{self, RuntimeTags};
use crate::watcher::{Change, Watcher};

const HOT_RELOAD_CHANNEL_CAPACITY: usize = 256;

pub struct AppState {
    project: RwLock<Project>,
    manifest: RwLock<Manifest>,
    revision: AtomicU64,
    hub: broadcast::Sender<String>,
    last_sync: RwLock<Option<serde_json::Value>>,
    base_url: String,
}

impl AppState {
    fn new(project: Project) -> Result<Arc<Self>> {
        let manifest = Manifest::build(&project.slides_dir(), 1)?;
        let (hub, _) = broadcast::channel(HOT_RELOAD_CHANNEL_CAPACITY);
        Ok(Arc::new(Self {
            project: RwLock::new(project),
            manifest: RwLock::new(manifest),
            revision: AtomicU64::new(1),
            hub,
            last_sync: RwLock::new(None),
            base_url: "/".to_owned(),
        }))
    }

    pub fn project(&self) -> Project {
        self.project.read().expect("project lock").clone()
    }

    pub fn manifest(&self) -> Manifest {
        self.manifest.read().expect("manifest lock").clone()
    }

    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::Relaxed)
    }

    fn bump_revision(&self) -> u64 {
        self.revision.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn rebuild_manifest(&self) -> Result<Manifest> {
        let project = self.project();
        let revision = self.bump_revision();
        let manifest = Manifest::build(&project.slides_dir(), revision)?;
        *self.manifest.write().expect("manifest lock") = manifest.clone();
        Ok(manifest)
    }

    fn reload_config(&self) -> Result<()> {
        let current = self.project();
        let reloaded = Project::open(Some(current.root()), None, &Overrides::default())?;
        *self.project.write().expect("project lock") = reloaded;
        Ok(())
    }

    fn broadcast(&self, message: serde_json::Value) {
        if let Ok(text) = serde_json::to_string(&message) {
            tracing::debug!("hot reload broadcast: {text}");
            let _ = self.hub.send(text);
        }
    }
}

/* -------------------------------------------------------------------------- */
/* server                                                                      */
/* -------------------------------------------------------------------------- */

pub struct Server {
    listener: TcpListener,
    state: Arc<AppState>,
    addr: SocketAddr,
}

impl Server {
    pub async fn bind(project: Project) -> Result<Self> {
        let host = project.config().server.host.clone();
        let port = project.config().server.port;
        let state = AppState::new(project)?;

        let listener = TcpListener::bind((host.as_str(), port))
            .await
            .map_err(|error| Error::config(format!("{host}:{port} にbindできません: {error}")))?;
        let addr = listener
            .local_addr()
            .map_err(|error| Error::config(format!("ローカルアドレスを取得できません: {error}")))?;

        Ok(Self { listener, state, addr })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// `http://host:port` for this server.
    pub fn origin(&self) -> String {
        let host = self.addr.ip();
        if self.addr.ip().is_unspecified() {
            format!("http://127.0.0.1:{}", self.addr.port())
        } else {
            format!("http://{host}:{}", self.addr.port())
        }
    }

    pub fn state(&self) -> Arc<AppState> {
        Arc::clone(&self.state)
    }

    /// Start watching the project and broadcasting hot reload events.
    pub fn spawn_watcher(&self) -> Result<()> {
        let state = Arc::clone(&self.state);
        let watcher = Watcher::start(&state.project())?;
        tokio::spawn(watch_loop(state, watcher));
        Ok(())
    }

    pub async fn serve(self) -> Result<()> {
        let app = router(Arc::clone(&self.state));
        axum::serve(self.listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .map_err(|error| Error::render(format!("サーバーが停止しました: {error}")))
    }

    /// Run the server on a background task (used by `deck check`).
    pub fn spawn(self) -> tokio::task::JoinHandle<Result<()>> {
        tokio::spawn(self.serve())
    }
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("停止します");
}

async fn watch_loop(state: Arc<AppState>, mut watcher: Watcher) {
    while let Some(changes) = watcher.changes.recv().await {
        for change in changes {
            let revision = state.bump_revision();
            match change {
                Change::Slide { path } => {
                    let slide_relative = path.strip_prefix("slides/").unwrap_or(&path).to_owned();
                    // Re-read the document so titles, notes and step counts stay fresh.
                    let manifest = match state.rebuild_manifest() {
                        Ok(manifest) => manifest,
                        Err(error) => {
                            tracing::error!("manifest を更新できません: {error}");
                            continue;
                        }
                    };
                    let slide_id = manifest
                        .slide_by_path(&slide_relative)
                        .map(|slide| slide.id.clone())
                        .unwrap_or_else(|| crate::discovery::id_from_path(&slide_relative));
                    state.broadcast(serde_json::json!({
                        "type": "slide-changed",
                        "revision": revision,
                        "path": path,
                        "slideId": slide_id,
                    }));
                }
                Change::SlideSet => match state.rebuild_manifest() {
                    Ok(manifest) => state.broadcast(serde_json::json!({
                        "type": "manifest-changed",
                        "revision": manifest.revision,
                        "slides": manifest.slides,
                    })),
                    Err(error) => tracing::error!("manifest を更新できません: {error}"),
                },
                Change::TailwindEntry { path } => state.broadcast(serde_json::json!({
                    "type": "tailwind-changed",
                    "revision": revision,
                    "path": path,
                })),
                Change::Style { path } => state.broadcast(serde_json::json!({
                    "type": "style-changed",
                    "revision": revision,
                    "path": path,
                })),
                Change::Component { path, tags } => state.broadcast(serde_json::json!({
                    "type": "component-changed",
                    "revision": revision,
                    "path": path,
                    "tags": tags,
                })),
                Change::Asset { path } => state.broadcast(serde_json::json!({
                    "type": "asset-changed",
                    "revision": revision,
                    "path": path,
                })),
                Change::Config { .. } => {
                    if let Err(error) = state.reload_config() {
                        tracing::error!("deck.toml を再読込できません: {error}");
                        state.broadcast(serde_json::json!({
                            "type": "error",
                            "message": error.to_string(),
                        }));
                        continue;
                    }
                    if let Err(error) = state.rebuild_manifest() {
                        tracing::error!("manifest を更新できません: {error}");
                    }
                    state.broadcast(serde_json::json!({
                        "type": "config-changed",
                        "revision": revision,
                    }));
                }
            }
        }
    }
}

/* -------------------------------------------------------------------------- */
/* routes                                                                      */
/* -------------------------------------------------------------------------- */

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(page_index))
        .route("/present", get(page_present))
        .route("/presenter", get(page_presenter))
        .route("/print", get(page_print))
        .route("/slides/{*rest}", get(slide_handler))
        .route("/@deck/manifest.json", get(manifest_handler))
        .route("/@deck/design.css", get(design_handler))
        .route("/@deck/components.js", get(components_handler))
        .route("/@deck/env.js", get(env_handler))
        .route("/@deck/tailwind.css", get(tailwind_handler))
        .route("/@deck/{*path}", get(deck_asset_handler))
        .route("/assets/{*path}", get(assets_handler))
        .route("/components/{*path}", get(components_dir_handler))
        .route("/design/{*path}", get(design_dir_handler))
        .route("/ws", get(ws_handler))
        .fallback(not_found)
        .with_state(state)
}

struct AppError(Error);

impl From<Error> for AppError {
    fn from(error: Error) -> Self {
        Self(error)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!("{}", self.0);
        (StatusCode::INTERNAL_SERVER_ERROR, format!("deck error: {}", self.0)).into_response()
    }
}

fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store, must-revalidate"));
    response
}

fn body_response(mime: &str, body: impl Into<Body>) -> Response {
    let mut response = Response::new(body.into());
    if let Ok(value) = HeaderValue::from_str(mime) {
        response.headers_mut().insert(header::CONTENT_TYPE, value);
    }
    no_store(response)
}

async fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "not found").into_response()
}

async fn page_index(State(state): State<Arc<AppState>>) -> Response {
    body_response("text/html; charset=utf-8", Page::Index.render(&state.base_url))
}

async fn page_present(State(state): State<Arc<AppState>>) -> Response {
    body_response("text/html; charset=utf-8", Page::Present.render(&state.base_url))
}

async fn page_presenter(State(state): State<Arc<AppState>>) -> Response {
    body_response("text/html; charset=utf-8", Page::Presenter.render(&state.base_url))
}

async fn page_print(State(state): State<Arc<AppState>>) -> Response {
    body_response("text/html; charset=utf-8", Page::Print.render(&state.base_url))
}

async fn manifest_handler(State(state): State<Arc<AppState>>) -> Response {
    body_response("application/json; charset=utf-8", state.manifest().to_json())
}

async fn design_handler(
    State(state): State<Arc<AppState>>,
) -> std::result::Result<Response, AppError> {
    let css = assets::design_css(&state.project())?;
    Ok(body_response("text/css; charset=utf-8", css))
}

async fn components_handler(State(state): State<Arc<AppState>>) -> Response {
    let js = assets::components_js(&state.project(), &state.base_url);
    body_response("text/javascript; charset=utf-8", js)
}

/// Exposed for debugging; slides receive this inline as `text/tailwindcss`.
async fn tailwind_handler(
    State(state): State<Arc<AppState>>,
) -> std::result::Result<Response, AppError> {
    let css = assets::tailwind_input(&state.project())?;
    Ok(body_response("text/css; charset=utf-8", css))
}

async fn env_handler(State(state): State<Arc<AppState>>) -> Response {
    let js = assets::env_module(state.project().config());
    body_response("text/javascript; charset=utf-8", js)
}

async fn deck_asset_handler(Path(path): Path<String>) -> Response {
    match assets::embedded(&path) {
        Some(bytes) => body_response(assets::mime_for(&path), Body::from(bytes)),
        None => (StatusCode::NOT_FOUND, format!("/@deck/{path} は存在しません")).into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct SlideQuery {
    #[serde(rename = "deck-mode")]
    _mode: Option<String>,
}

async fn slide_handler(
    State(state): State<Arc<AppState>>,
    Path(rest): Path<String>,
    Query(_query): Query<SlideQuery>,
) -> Response {
    let project = state.project();
    let slides_dir = project.slides_dir();
    let rest = decode_path(&rest);

    if !is_safe_relative(&rest) {
        return (StatusCode::BAD_REQUEST, "不正なパスです").into_response();
    }

    let tailwind = match assets::tailwind_input(&project) {
        Ok(css) => css,
        Err(error) => return AppError(error).into_response(),
    };
    let tags = RuntimeTags { base_url: &state.base_url, tailwind_css: &tailwind };

    // 1. stable slide id
    if let Some(slide) = state.manifest().slide(&rest) {
        return serve_slide(&slides_dir.join(&slide.path), &tags);
    }
    // 2. a file colocated with the slides (images, partial CSS, ...)
    let direct = slides_dir.join(&rest);
    if direct.is_file() {
        return if direct.extension() == Some("html") {
            serve_slide(&direct, &tags)
        } else {
            serve_file(&direct)
        };
    }
    // 3. path without the .html extension
    let with_extension = slides_dir.join(format!("{rest}.html"));
    if with_extension.is_file() {
        return serve_slide(&with_extension, &tags);
    }

    (StatusCode::NOT_FOUND, format!("slide が見つかりません: {rest}")).into_response()
}

fn serve_slide(path: &Utf8Path, tags: &RuntimeTags<'_>) -> Response {
    match std::fs::read_to_string(path) {
        Ok(source) => {
            body_response("text/html; charset=utf-8", render::ensure_runtime_tags(&source, tags))
        }
        Err(error) => {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("{path}: {error}")).into_response()
        }
    }
}

fn serve_file(path: &Utf8Path) -> Response {
    match std::fs::read(path) {
        Ok(bytes) => {
            let mime = mime_guess::from_path(path.as_std_path()).first_or_octet_stream();
            body_response(mime.as_ref(), Body::from(bytes))
        }
        Err(_) => (StatusCode::NOT_FOUND, format!("{path} は存在しません")).into_response(),
    }
}

async fn assets_handler(State(state): State<Arc<AppState>>, Path(path): Path<String>) -> Response {
    serve_from_dir(&state.project().assets_dir(), &path)
}

async fn components_dir_handler(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
) -> Response {
    serve_from_dir(&state.project().components_dir(), &path)
}

async fn design_dir_handler(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
) -> Response {
    serve_from_dir(&state.project().design_dir(), &path)
}

fn serve_from_dir(dir: &Utf8Path, path: &str) -> Response {
    let path = decode_path(path);
    if !is_safe_relative(&path) {
        return (StatusCode::BAD_REQUEST, "不正なパスです").into_response();
    }
    let target = dir.join(&path);
    if target.is_file() {
        serve_file(&target)
    } else {
        (StatusCode::NOT_FOUND, "not found").into_response()
    }
}

fn decode_path(path: &str) -> String {
    percent_encoding::percent_decode_str(path).decode_utf8_lossy().into_owned()
}

/// Reject absolute paths and `..` so a request cannot escape the project.
fn is_safe_relative(path: &str) -> bool {
    let candidate = Utf8PathBuf::from(path);
    candidate.is_relative()
        && candidate
            .components()
            .all(|component| !matches!(component, camino::Utf8Component::ParentDir))
        && !path.contains('\0')
}

/* -------------------------------------------------------------------------- */
/* websocket                                                                   */
/* -------------------------------------------------------------------------- */

async fn ws_handler(State(state): State<Arc<AppState>>, upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(move |socket| ws_session(state, socket))
}

async fn ws_session(state: Arc<AppState>, socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();
    let mut updates = state.hub.subscribe();

    let hello = serde_json::json!({
        "type": "hello",
        "revision": state.revision(),
        "sync": state.last_sync.read().expect("sync lock").clone(),
    });
    if sender.send(Message::Text(hello.to_string().into())).await.is_err() {
        return;
    }

    let outgoing = tokio::spawn(async move {
        while let Ok(text) = updates.recv().await {
            if sender.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(message)) = receiver.next().await {
        let Message::Text(text) = message else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        match value.get("type").and_then(serde_json::Value::as_str) {
            // Presenter/audience synchronisation is relayed to every other client.
            Some("sync") => {
                *state.last_sync.write().expect("sync lock") = Some(value.clone());
                let _ = state.hub.send(text.to_string());
            }
            Some("hello") => {}
            _ => {}
        }
    }

    outgoing.abort();
}
