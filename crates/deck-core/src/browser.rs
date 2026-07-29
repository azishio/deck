//! headless Chromium driver (design doc 4.5, 16, 20).
//!
//! Only Chromium is supported and there is deliberately no browser abstraction
//! layer.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use chromiumoxide::browser::{Browser, BrowserConfig as LaunchConfig};
use chromiumoxide::cdp::browser_protocol::emulation::{
    SetLocaleOverrideParams, SetTimezoneOverrideParams,
};
use chromiumoxide::cdp::browser_protocol::network::{EventLoadingFailed, EventRequestWillBeSent};
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
use chromiumoxide::cdp::js_protocol::runtime::EventExceptionThrown;
use chromiumoxide::handler::viewport::Viewport;
use chromiumoxide::page::{Page, ScreenshotParams};
use futures::StreamExt;
use tokio::task::JoinHandle;

use crate::config::{BrowserConfig, Canvas};
use crate::error::{Error, Result};

/// Values pinned so check results stay reproducible (design doc 20).
const TIMEZONE: &str = "UTC";
const LOCALE: &str = "ja-JP";

pub struct BrowserSession {
    browser: Browser,
    handler: JoinHandle<()>,
    user_data_dir: camino::Utf8PathBuf,
}

/// Resolve a browser command to an executable: an absolute path is used as
/// given, a bare name is looked up in `PATH`.
pub fn locate_browser(command: &str) -> Option<camino::Utf8PathBuf> {
    if command.is_empty() {
        return None;
    }
    let candidate = camino::Utf8Path::new(command);
    if candidate.is_absolute() {
        return candidate.is_file().then(|| candidate.to_path_buf());
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(command))
        .find(|candidate| candidate.is_file())
        .and_then(|found| camino::Utf8PathBuf::from_path_buf(found).ok())
}

/// Chromium refuses to share a profile directory between processes, so each
/// session gets its own and removes it on close.
fn unique_user_data_dir() -> camino::Utf8PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    let base = camino::Utf8PathBuf::from_path_buf(std::env::temp_dir())
        .unwrap_or_else(|_| camino::Utf8PathBuf::from("/tmp"));
    base.join(format!("deck-chromium-{}-{sequence}", std::process::id()))
}

impl BrowserSession {
    pub async fn launch(config: &BrowserConfig, canvas: Canvas) -> Result<Self> {
        let viewport = Viewport {
            width: canvas.width,
            height: canvas.height,
            device_scale_factor: Some(1.0),
            ..Viewport::default()
        };

        let user_data_dir = unique_user_data_dir();
        std::fs::create_dir_all(&user_data_dir)
            .map_err(|error| Error::io(&user_data_dir, error))?;

        let mut builder = LaunchConfig::builder()
            .user_data_dir(&user_data_dir)
            .window_size(canvas.width, canvas.height)
            .viewport(viewport)
            .launch_timeout(Duration::from_secs(30))
            .args([
                "--hide-scrollbars",
                "--force-device-scale-factor=1",
                "--force-color-profile=srgb",
                "--font-render-hinting=none",
                "--disable-lcd-text",
                "--disable-background-timer-throttling",
                "--disable-renderer-backgrounding",
                "--js-flags=--random-seed=1",
                "--no-first-run",
                "--no-default-browser-check",
            ]);

        // chromiumoxide needs a path, not a command name, so `command` is
        // resolved through PATH first. Falling through leaves its own detection
        // in charge, which is what the default "chromium" relies on.
        if let Some(executable) = locate_browser(&config.command) {
            builder = builder.chrome_executable(executable.as_std_path());
        } else if config.command != "chromium" {
            return Err(Error::browser(format!(
                "{} was not found. Set [browser] command in deck.local.toml to the executable",
                config.command
            )));
        }
        builder = if config.headless { builder.new_headless_mode() } else { builder.with_head() };
        if !config.sandbox {
            builder = builder.no_sandbox();
        }

        let launch = builder
            .build()
            .map_err(|error| Error::browser(format!("could not configure Chromium: {error}")))?;

        let (browser, mut events) = Browser::launch(launch).await.map_err(|error| {
            let message = error.to_string();
            let hint = if config.sandbox && message.contains("sandbox") {
                "\nChromium's sandbox is unavailable here. Set [browser] sandbox = false in deck.local.toml"
            } else {
                "\nRun `deck doctor` to check the executable"
            };
            Error::browser(format!("could not launch Chromium ({}): {message}{hint}", config.command))
        })?;

        let handler = tokio::spawn(async move { while events.next().await.is_some() {} });

        Ok(Self { browser, handler, user_data_dir })
    }

    pub async fn version(&self) -> Result<String> {
        let version = self
            .browser
            .version()
            .await
            .map_err(|error| Error::browser(format!("could not read the version: {error}")))?;
        Ok(version.product)
    }

    /// Open `url` in a fresh page with event capture enabled.
    pub async fn open(&self, url: &str) -> Result<PageSession> {
        let page = self
            .browser
            .new_page("about:blank")
            .await
            .map_err(|error| Error::browser(format!("could not create a page: {error}")))?;

        page.emulate_timezone(SetTimezoneOverrideParams::new(TIMEZONE)).await.ok();
        page.emulate_locale(SetLocaleOverrideParams::builder().locale(LOCALE).build()).await.ok();
        page.enable_runtime().await.map_err(|error| {
            Error::browser(format!("could not enable the Runtime domain: {error}"))
        })?;

        let events = Arc::new(Mutex::new(PageEvents::default()));
        let listeners = spawn_listeners(&page, Arc::clone(&events)).await?;

        page.goto(url)
            .await
            .map_err(|error| Error::browser(format!("could not open {url}: {error}")))?;

        Ok(PageSession { page, events, listeners })
    }

    pub async fn close(mut self) {
        let _ = self.browser.close().await;
        let _ = self.browser.wait().await;
        self.handler.abort();
        let _ = std::fs::remove_dir_all(&self.user_data_dir);
    }
}

/* -------------------------------------------------------------------------- */
/* page                                                                        */
/* -------------------------------------------------------------------------- */

#[derive(Debug, Default, Clone)]
pub struct PageEvents {
    pub exceptions: Vec<String>,
    /// `(url, error text)` for requests that never completed.
    pub failed_requests: Vec<(String, String)>,
    pub requests: Vec<String>,
}

/// Request ids seen so far, so a failure can be reported with its URL.
#[derive(Debug, Default)]
struct RequestIndex(std::collections::HashMap<String, String>);

pub struct PageSession {
    page: Page,
    events: Arc<Mutex<PageEvents>>,
    listeners: Vec<JoinHandle<()>>,
}

impl PageSession {
    pub fn page(&self) -> &Page {
        &self.page
    }

    pub fn events(&self) -> PageEvents {
        self.events.lock().expect("events lock").clone()
    }

    /// Evaluate an expression and deserialize its result.
    pub async fn evaluate<T: serde::de::DeserializeOwned>(&self, expression: &str) -> Result<T> {
        let result =
            self.page.evaluate_expression(expression).await.map_err(|error| {
                Error::browser(format!("could not evaluate the script: {error}"))
            })?;
        result.into_value().map_err(|error| {
            Error::browser(format!("could not read the evaluation result: {error}"))
        })
    }

    /// Poll `expression` until it evaluates to `true` or the timeout elapses.
    pub async fn wait_for(&self, expression: &str, timeout: Duration) -> Result<bool> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if let Ok(true) = self.evaluate::<bool>(expression).await {
                return Ok(true);
            }
            if tokio::time::Instant::now() >= deadline {
                return Ok(false);
            }
            tokio::time::sleep(Duration::from_millis(60)).await;
        }
    }

    /// Click at a viewport coordinate with a real input event.
    pub async fn click_at(&self, x: f64, y: f64) -> Result<()> {
        self.page
            .click(chromiumoxide::layout::Point::new(x, y))
            .await
            .map_err(|error| Error::browser(format!("could not click at ({x}, {y}): {error}")))?;
        Ok(())
    }

    pub async fn screenshot(&self, path: &camino::Utf8Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| Error::io(parent, error))?;
        }
        let params = ScreenshotParams::builder().format(CaptureScreenshotFormat::Png).build();
        self.page
            .save_screenshot(params, path.as_std_path())
            .await
            .map_err(|error| Error::browser(format!("could not save the screenshot: {error}")))?;
        Ok(())
    }

    pub async fn close(self) {
        for listener in self.listeners {
            listener.abort();
        }
        let _ = self.page.close().await;
    }
}

async fn spawn_listeners(
    page: &Page,
    events: Arc<Mutex<PageEvents>>,
) -> Result<Vec<JoinHandle<()>>> {
    let mut handles = Vec::new();

    let mut exceptions = page.event_listener::<EventExceptionThrown>().await.map_err(|error| {
        Error::browser(format!("could not subscribe to exception events: {error}"))
    })?;
    let sink = Arc::clone(&events);
    handles.push(tokio::spawn(async move {
        while let Some(event) = exceptions.next().await {
            let details = &event.exception_details;
            let message = details
                .exception
                .as_ref()
                .and_then(|value| value.description.clone())
                .unwrap_or_else(|| details.text.clone());
            sink.lock().expect("events lock").exceptions.push(message);
        }
    }));

    let index = Arc::new(Mutex::new(RequestIndex::default()));

    let mut requests = page.event_listener::<EventRequestWillBeSent>().await.map_err(|error| {
        Error::browser(format!("could not subscribe to request events: {error}"))
    })?;
    let sink = Arc::clone(&events);
    let request_index = Arc::clone(&index);
    handles.push(tokio::spawn(async move {
        while let Some(event) = requests.next().await {
            let url = event.request.url.clone();
            request_index
                .lock()
                .expect("request index")
                .0
                .insert(event.request_id.inner().clone(), url.clone());
            sink.lock().expect("events lock").requests.push(url);
        }
    }));

    let mut failures = page.event_listener::<EventLoadingFailed>().await.map_err(|error| {
        Error::browser(format!("could not subscribe to network events: {error}"))
    })?;
    let sink = Arc::clone(&events);
    let request_index = Arc::clone(&index);
    handles.push(tokio::spawn(async move {
        while let Some(event) = failures.next().await {
            let url = request_index
                .lock()
                .expect("request index")
                .0
                .get(event.request_id.inner())
                .cloned()
                .unwrap_or_else(|| event.request_id.inner().clone());
            sink.lock().expect("events lock").failed_requests.push((url, event.error_text.clone()));
        }
    }));

    Ok(handles)
}
