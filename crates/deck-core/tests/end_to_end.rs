//! End-to-end checks against the acceptance criteria (design doc 22).
//!
//! The browser-driven tests skip themselves when no Chromium is available, so
//! the suite still runs on a machine without one.

use std::time::Duration;

use camino::Utf8PathBuf;
use deck_core::browser::BrowserSession;
use deck_core::config::Overrides;
use deck_core::project::Project;
use deck_core::scaffold::{self, Theme};
use deck_core::server::Server;

struct TempProject {
    root: Utf8PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let root = Utf8PathBuf::from_path_buf(std::env::temp_dir())
            .expect("temp dir is UTF-8")
            .join(format!("deck-e2e-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        scaffold::init(&root, "E2E Deck", Theme::Default).expect("scaffold");
        Self { root }
    }

    fn open(&self) -> Project {
        Project::open(Some(&self.root), None, &Overrides::default()).expect("open project")
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn chromium_available(project: &Project) -> bool {
    deck_core::browser::locate_browser(&project.config().browser.command).is_some()
}

/// Poll `expression` until it stops returning `null`/`undefined`.
async fn wait_value<T: serde::de::DeserializeOwned>(
    page: &deck_core::browser::PageSession,
    expression: &str,
    timeout: Duration,
) -> Option<T> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Ok(value) = page.evaluate::<T>(expression).await {
            return Some(value);
        }
        if tokio::time::Instant::now() >= deadline {
            return None;
        }
        tokio::time::sleep(Duration::from_millis(80)).await;
    }
}

#[tokio::test]
async fn manifest_is_discovered_from_the_slides_directory() {
    let temp = TempProject::new("manifest");
    let project = temp.open();
    let server = Server::bind(project).await.expect("bind");
    let origin = server.origin();
    let _task = server.spawn();

    let manifest: serde_json::Value =
        reqwest_get_json(&format!("{origin}/@deck/manifest.json")).await;
    let slides = manifest["slides"].as_array().expect("slides");

    assert_eq!(slides.len(), 3, "scaffolded deck has three slides");
    assert_eq!(slides[0]["id"], "title");
    assert_eq!(slides[1]["id"], "overview");
    assert_eq!(slides[1]["stepCount"], 3);
    // Order follows the file names, not the order the files were written.
    assert_eq!(slides[2]["path"], "20-architecture.html");
}

/// Minimal HTTP GET so the tests do not need an HTTP client dependency.
async fn reqwest_get_json(url: &str) -> serde_json::Value {
    let body = http_get(url).await;
    serde_json::from_str(&body).expect("json body")
}

async fn http_get(url: &str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let rest = url.strip_prefix("http://").expect("http url");
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let mut stream = tokio::net::TcpStream::connect(authority).await.expect("connect");
    stream
        .write_all(format!("GET /{path} HTTP/1.0\r\nHost: {authority}\r\n\r\n").as_bytes())
        .await
        .expect("write request");

    let mut response = String::new();
    stream.read_to_string(&mut response).await.expect("read response");
    let (head, body) = response.split_once("\r\n\r\n").expect("headers");
    assert!(head.starts_with("HTTP/1.0 200"), "unexpected response: {head}");
    body.to_owned()
}

#[tokio::test]
async fn a_slide_renders_and_reaches_ready() {
    let temp = TempProject::new("ready");
    let project = temp.open();
    if !chromium_available(&project) {
        eprintln!("Chromium が無いため skip します");
        return;
    }

    let canvas = project.config().canvas;
    let browser_config = project.config().browser.clone();
    let server = Server::bind(project).await.expect("bind");
    let origin = server.origin();
    let _task = server.spawn();

    let browser = BrowserSession::launch(&browser_config, canvas).await.expect("launch");
    let page = browser.open(&format!("{origin}/slides/overview")).await.expect("open");

    let ready = page
        .wait_for("document.documentElement.dataset.deckReady === 'true'", Duration::from_secs(15))
        .await
        .expect("evaluate");
    assert!(ready, "slide never became ready");

    // Tailwind compiled inside the slide document.
    let tailwind: bool = page
        .evaluate(
            "[...document.querySelectorAll('style:not([type])')]\
             .some((s) => s.textContent.includes('@layer theme'))",
        )
        .await
        .expect("evaluate");
    assert!(tailwind, "Tailwind CSS did not compile in the slide");

    // The design system applies: deck-slide keeps its padding despite preflight.
    let padding: String = page
        .evaluate("getComputedStyle(document.querySelector('deck-slide')).paddingLeft")
        .await
        .expect("evaluate");
    assert_eq!(padding, "64px", "preflight must not outrank the deck design system");

    let step_count: u32 = page.evaluate("window.deck.stepCount").await.expect("evaluate");
    assert_eq!(step_count, 3);

    page.close().await;
    browser.close().await;
}

/// Writes a slide whose only content is a slow `countup` stat.
fn write_stat_slide(project: &Project, file_name: &str, step: Option<u32>) {
    let step_attribute = step.map(|step| format!(" data-step=\"{step}\"")).unwrap_or_default();
    std::fs::write(
        project.slides_dir().join(file_name),
        format!(
            r#"<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>Stat</title>
<link rel="stylesheet" href="/@deck/design.css">
<script type="module" src="/@deck/boot.js"></script></head>
<body>
  <deck-slide id="stat">
    <deck-stat countup countup-duration="4000"{step_attribute}>
      <span>1280</span>
      <span>logical canvas width</span>
    </deck-stat>
  </deck-slide>
</body>
</html>
"#
        ),
    )
    .expect("write slide");
}

/// The state of the stat inside a named frame, whether or not it is current.
fn stat_state(slide_id: &str) -> String {
    format!(
        "(window.deckShell.frames.get('{slide_id}')?.iframe.contentDocument\
         ?.querySelector('deck-stat')?.dataset.deckCountup ?? null)"
    )
}

/// A slide preloaded by the iframe ring must not start its reveal animations
/// before it is actually shown, and must replay them on every re-entry.
///
/// Animating from `connectedCallback` looked fine on a first visit and then
/// re-triggered seemingly at random, because whether a slide had already been
/// constructed depended on how far away the previous slide was.
#[tokio::test]
async fn a_preloaded_slide_animates_only_once_it_is_entered() {
    let temp = TempProject::new("preload");
    let project = temp.open();
    if !chromium_available(&project) {
        eprintln!("Chromium が無いため skip します");
        return;
    }

    // 05- sorts right after the title slide, so the ring preloads it.
    write_stat_slide(&project, "05-stat.html", None);

    let canvas = project.config().canvas;
    let browser_config = project.config().browser.clone();
    let server = Server::bind(project).await.expect("bind");
    let origin = server.origin();
    let _task = server.spawn();

    let browser = BrowserSession::launch(&browser_config, canvas).await.expect("launch");
    let page = browser.open(&format!("{origin}/present#/title/0")).await.expect("open");

    let state = stat_state("stat");
    assert!(
        page.wait_for(&format!("{state} !== null"), Duration::from_secs(15))
            .await
            .expect("evaluate"),
        "the neighbouring slide was never preloaded"
    );

    // Preloaded but off-screen: nothing may have started yet.
    let idle: String = page.evaluate(&state).await.expect("evaluate");
    assert_eq!(idle, "idle", "a preloaded slide must not animate before it is shown");

    for attempt in 1..=2 {
        page.evaluate::<serde_json::Value>("window.deckShell.goToSlideId('stat'), null").await.ok();
        assert!(
            page.wait_for(&format!("{state} === 'running'"), Duration::from_secs(5))
                .await
                .expect("evaluate"),
            "entering the slide did not start the animation on attempt {attempt}"
        );

        page.evaluate::<serde_json::Value>("window.deckShell.goToSlideId('title'), null")
            .await
            .ok();
        assert!(
            page.wait_for(&format!("{state} === 'idle'"), Duration::from_secs(5))
                .await
                .expect("evaluate"),
            "leaving the slide did not re-arm the animation on attempt {attempt}"
        );
    }

    page.close().await;
    browser.close().await;
}

/// A `countup` stat follows the step model: stepping away and back replays it,
/// exactly like the standard reveal animation.
#[tokio::test]
async fn countup_replays_when_the_stat_becomes_visible_again() {
    let temp = TempProject::new("countup");
    let project = temp.open();
    if !chromium_available(&project) {
        eprintln!("Chromium が無いため skip します");
        return;
    }

    // A deliberately slow count-up, so "running" is observable.
    write_stat_slide(&project, "30-stat.html", Some(1));

    let canvas = project.config().canvas;
    let browser_config = project.config().browser.clone();
    let server = Server::bind(project).await.expect("bind");
    let origin = server.origin();
    let _task = server.spawn();

    let browser = BrowserSession::launch(&browser_config, canvas).await.expect("launch");
    let page = browser.open(&format!("{origin}/present#/stat/0")).await.expect("open");

    assert!(
        page.wait_for("window.deckShell?.slide?.id === 'stat'", Duration::from_secs(15))
            .await
            .expect("evaluate"),
        "never reached the stat slide"
    );

    // Parenthesised: `??` binds looser than `===`, so the comparison has to be
    // applied to the whole lookup.
    const STATE: &str = "(window.deckShell.currentFrame()?.iframe.contentDocument\
         ?.querySelector('deck-stat')?.dataset.deckCountup ?? null)";

    // Hidden at step 0, so the animation has not started.
    assert!(
        page.wait_for(&format!("{STATE} === 'idle'"), Duration::from_secs(15))
            .await
            .expect("evaluate"),
        "count-up must wait until the stat is revealed"
    );

    for attempt in 1..=2 {
        page.evaluate::<serde_json::Value>("window.deckShell.setStep(1), null").await.ok();
        assert!(
            page.wait_for(&format!("{STATE} === 'running'"), Duration::from_secs(5))
                .await
                .expect("evaluate"),
            "count-up did not start on attempt {attempt}"
        );

        page.evaluate::<serde_json::Value>("window.deckShell.setStep(0), null").await.ok();
        assert!(
            page.wait_for(&format!("{STATE} === 'idle'"), Duration::from_secs(5))
                .await
                .expect("evaluate"),
            "count-up did not re-arm on attempt {attempt}"
        );
    }

    page.close().await;
    browser.close().await;
}

#[tokio::test]
async fn presentation_navigates_steps_and_survives_hot_reload() {
    let temp = TempProject::new("hot");
    let project = temp.open();
    if !chromium_available(&project) {
        eprintln!("Chromium が無いため skip します");
        return;
    }

    let canvas = project.config().canvas;
    let browser_config = project.config().browser.clone();
    let slide_path = project.slides_dir().join("10-overview.html");

    let server = Server::bind(project).await.expect("bind");
    let origin = server.origin();
    server.spawn_watcher().expect("watcher");
    let _task = server.spawn();

    let browser = BrowserSession::launch(&browser_config, canvas).await.expect("launch");
    let page = browser.open(&format!("{origin}/present")).await.expect("open");

    let started = page
        .wait_for("Boolean(window.deckShell?.slides?.length)", Duration::from_secs(15))
        .await
        .expect("evaluate");
    assert!(started, "shell never loaded the manifest");

    // Move to the second slide and advance two steps.
    page.evaluate::<serde_json::Value>("window.deckShell.goToSlideId('overview', 0), null")
        .await
        .ok();
    page.evaluate::<serde_json::Value>("window.deckShell.next(), window.deckShell.next(), null")
        .await
        .ok();

    let hash: String = wait_value(&page, "location.hash", Duration::from_secs(5)).await.unwrap();
    assert_eq!(hash, "#/overview/2", "URL fragment records slide and step");

    // Only the current slide and its neighbours are loaded.
    let frames: usize = page.evaluate("window.deckShell.frames.size").await.expect("evaluate");
    assert!(frames <= 3, "expected an iframe ring, found {frames} frames");

    // Edit the slide on disk; the shell must swap the iframe and keep the step.
    let source = std::fs::read_to_string(&slide_path).expect("read slide");
    std::fs::write(&slide_path, source.replace("素のHTML", "編集済みHTML")).expect("write slide");

    let reloaded = page
        .wait_for(
            "window.deckShell.currentFrame()?.iframe.contentDocument\
             ?.body?.textContent?.includes('編集済みHTML') === true",
            Duration::from_secs(20),
        )
        .await
        .expect("evaluate");
    assert!(reloaded, "hot reload never delivered the new slide content");

    let after: String = page.evaluate("location.hash").await.expect("evaluate");
    assert_eq!(after, "#/overview/2", "hot reload preserved the slide id and step");

    let step: u32 = page
        .evaluate("window.deckShell.currentFrame().iframe.contentWindow.deck.step")
        .await
        .expect("evaluate");
    assert_eq!(step, 2, "the replacement iframe was restored to the current step");

    page.close().await;
    browser.close().await;
}
