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
        eprintln!("skipping: no Chromium available");
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

/// A slide opened on its own is navigable: the right half of the page and the
/// arrow keys advance a step, and carry on to the adjacent slide once the steps
/// of the current one run out.
#[tokio::test]
async fn a_single_slide_page_navigates_by_click() {
    let temp = TempProject::new("standalone");
    let project = temp.open();
    if !chromium_available(&project) {
        eprintln!("skipping: no Chromium available");
        return;
    }

    let canvas = project.config().canvas;
    let browser_config = project.config().browser.clone();
    let server = Server::bind(project).await.expect("bind");
    let origin = server.origin();
    let _task = server.spawn();

    let browser = BrowserSession::launch(&browser_config, canvas).await.expect("launch");
    let page = browser.open(&format!("{origin}/slides/overview")).await.expect("open");
    assert!(
        page.wait_for(
            "document.documentElement.dataset.deckReady === 'true'",
            Duration::from_secs(15)
        )
        .await
        .expect("evaluate"),
        "slide never became ready"
    );

    // Right half: one step forward.
    page.click_at(1000.0, 400.0).await.expect("click");
    assert!(
        page.wait_for("window.deck.step === 1", Duration::from_secs(5)).await.expect("evaluate"),
        "clicking the right half did not advance a step"
    );

    // Left half: back again.
    page.click_at(200.0, 400.0).await.expect("click");
    assert!(
        page.wait_for("window.deck.step === 0", Duration::from_secs(5)).await.expect("evaluate"),
        "clicking the left half did not go back a step"
    );

    // Past the last step, the click moves to the next slide.
    page.evaluate::<serde_json::Value>("window.deck.goToStep(window.deck.stepCount), null")
        .await
        .ok();
    page.click_at(1000.0, 400.0).await.expect("click");
    assert!(
        page.wait_for(
            "location.pathname.endsWith('/slides/architecture')",
            Duration::from_secs(10)
        )
        .await
        .expect("evaluate"),
        "the last step did not carry on to the next slide"
    );

    // And back the other way, landing on the previous slide's final step.
    assert!(
        page.wait_for(
            "document.documentElement.dataset.deckReady === 'true'",
            Duration::from_secs(15)
        )
        .await
        .expect("evaluate"),
        "the next slide never became ready"
    );
    page.click_at(200.0, 400.0).await.expect("click");
    assert!(
        page.wait_for("location.pathname.endsWith('/slides/overview')", Duration::from_secs(10))
            .await
            .expect("evaluate"),
        "the first step did not carry back to the previous slide"
    );
    assert!(
        page.wait_for(
            "window.deck.step === window.deck.stepCount && window.deck.stepCount > 0",
            Duration::from_secs(15)
        )
        .await
        .expect("evaluate"),
        "going back should land on the previous slide's final step"
    );

    page.close().await;
    browser.close().await;
}

/// A slide is a whole web page, so a control on one has to receive the input
/// that operates it.
///
/// Navigation lives inside the slide document — a click on the right half
/// advances, arrow keys step — which means an unguarded runtime eats the very
/// clicks and keys an interactive slide is made of.
#[tokio::test]
async fn an_interactive_slide_keeps_the_input_it_needs() {
    let temp = TempProject::new("interactive");
    let project = temp.open();
    if !chromium_available(&project) {
        eprintln!("skipping: no Chromium available");
        return;
    }

    std::fs::write(
        project.slides_dir().join("30-interactive.html"),
        r##"<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>Interactive</title>
<link rel="stylesheet" href="/@deck/design.css">
<script type="module" src="/@deck/boot.js"></script></head>
<body>
  <deck-slide id="interactive">
    <input id="dial" type="range" min="0" max="10" value="5"
           style="position:absolute;left:900px;top:80px;width:300px">
    <button id="tap" type="button"
            style="position:absolute;left:900px;top:200px;width:300px;height:60px">0</button>
    <div id="pad" data-deck-no-nav
         style="position:absolute;left:900px;top:320px;width:300px;height:200px"></div>
    <p data-step="1">revealed</p>
  </deck-slide>
  <script type="module">
    const tap = document.querySelector("#tap");
    tap.addEventListener("click", () => {
      tap.textContent = String(Number(tap.textContent) + 1);
    });
  </script>
</body>
</html>
"##,
    )
    .expect("write slide");

    let canvas = project.config().canvas;
    let browser_config = project.config().browser.clone();
    let server = Server::bind(project).await.expect("bind");
    let origin = server.origin();
    let _task = server.spawn();

    let browser = BrowserSession::launch(&browser_config, canvas).await.expect("launch");
    let page = browser.open(&format!("{origin}/slides/interactive")).await.expect("open");
    assert!(
        page.wait_for(
            "document.documentElement.dataset.deckReady === 'true'",
            Duration::from_secs(15)
        )
        .await
        .expect("evaluate"),
        "slide never became ready"
    );

    // A button on the right half is a button, not a page turn.
    page.click_at(1050.0, 230.0).await.expect("click");
    assert!(
        page.wait_for("document.querySelector('#tap').textContent === '1'", Duration::from_secs(5))
            .await
            .expect("evaluate"),
        "the button never saw its own click"
    );
    assert_eq!(
        page.evaluate::<u32>("window.deck.step").await.expect("evaluate"),
        0,
        "clicking a button must not advance the deck"
    );

    // Arrow keys belong to a focused slider, not to the deck.
    page.click_at(1050.0, 90.0).await.expect("click");
    page.press_key("ArrowRight").await.expect("press");
    assert!(
        page.wait_for("document.querySelector('#dial').value === '6'", Duration::from_secs(5))
            .await
            .expect("evaluate"),
        "the slider never moved"
    );
    assert_eq!(
        page.evaluate::<u32>("window.deck.step").await.expect("evaluate"),
        0,
        "a key the focused control needs must not step the deck"
    );

    // An opted-out region is the slide's, however plain its contents.
    page.click_at(1050.0, 420.0).await.expect("click");
    assert_eq!(
        page.evaluate::<u32>("window.deck.step").await.expect("evaluate"),
        0,
        "data-deck-no-nav did not hold the click"
    );

    // Everywhere else still navigates, which is the point of guarding narrowly.
    page.click_at(700.0, 600.0).await.expect("click");
    assert!(
        page.wait_for("window.deck.step === 1", Duration::from_secs(5)).await.expect("evaluate"),
        "plain background clicks must still advance"
    );
    page.press_key("ArrowLeft").await.expect("press");
    assert!(
        page.wait_for("window.deck.step === 0", Duration::from_secs(5)).await.expect("evaluate"),
        "arrow keys must still work outside a control"
    );

    page.close().await;
    browser.close().await;
}

/// A slide that animates its own reveals declares its steps instead of marking
/// them up, and that is not a mistake to report.
///
/// `step_count_mismatch` compares the runtime count against the `[data-step]`
/// elements, so before this it fired on every scene driven by `setStepCount()`
/// — which is the documented way to have steps without markup.
#[tokio::test]
async fn declaring_a_step_count_is_not_a_mismatch() {
    let temp = TempProject::new("declared-steps");
    let project = temp.open();
    if !chromium_available(&project) {
        eprintln!("skipping: no Chromium available");
        return;
    }

    std::fs::write(
        project.slides_dir().join("30-declared.html"),
        r#"<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>Declared</title>
<link rel="stylesheet" href="/@deck/design.css">
<script type="module" src="/@deck/boot.js"></script></head>
<body>
  <deck-slide id="declared"><p>Nothing here carries data-step.</p></deck-slide>
  <script type="module">window.deck.setStepCount(3);</script>
</body>
</html>
"#,
    )
    .expect("write slide");

    let report = deck_core::check::run(
        &project,
        &deck_core::check::CheckOptions {
            slides: vec!["declared".to_owned()],
            ..Default::default()
        },
    )
    .await
    .expect("check");

    let mismatches: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.rule == "step_count_mismatch")
        .collect();
    assert!(mismatches.is_empty(), "declaring the count should not be reported: {mismatches:?}");
    assert_eq!(
        report.diagnostics.iter().filter(|d| d.rule == "javascript_exception").count(),
        0,
        "the slide itself should be clean"
    );
}

/// A step driven from one client must animate on the others.
///
/// The presenter and the audience view are separate pages kept in step over a
/// websocket; the audience is the one people are watching, so applying a remote
/// step instantly made the view everyone sees the only one without animation.
#[tokio::test]
async fn a_remotely_driven_step_animates_on_the_other_client() {
    let temp = TempProject::new("sync");
    let project = temp.open();
    if !chromium_available(&project) {
        eprintln!("skipping: no Chromium available");
        return;
    }

    let canvas = project.config().canvas;
    let browser_config = project.config().browser.clone();
    let server = Server::bind(project).await.expect("bind");
    let origin = server.origin();
    let _task = server.spawn();

    let browser = BrowserSession::launch(&browser_config, canvas).await.expect("launch");
    // Two clients on the same deck, both sitting on a slide that has steps.
    let driver = browser.open(&format!("{origin}/present#/overview/0")).await.expect("open");
    let follower = browser.open(&format!("{origin}/presenter#/overview/0")).await.expect("open");

    for page in [&driver, &follower] {
        assert!(
            page.wait_for(
                "window.deckShell?.slide?.id === 'overview'                  && window.deckShell.currentFrame()?.ready === true",
                Duration::from_secs(20),
            )
            .await
            .expect("evaluate"),
            "a client never settled on the slide"
        );
    }

    // Record how the follower's slide is told to change step.
    follower
        .evaluate::<serde_json::Value>(
            "window.__instant = [],              window.deckShell.currentFrame().iframe.contentDocument                  .addEventListener('deck:stepchange', (e) => window.__instant.push(e.detail.instant)),              null",
        )
        .await
        .ok();

    driver.evaluate::<serde_json::Value>("window.deckShell.next(), null").await.ok();

    assert!(
        follower
            .wait_for("window.deckShell.step === 1", Duration::from_secs(10))
            .await
            .expect("evaluate"),
        "the follower never received the step"
    );

    let instants: Vec<bool> = follower.evaluate("window.__instant").await.expect("evaluate");
    assert_eq!(
        instants,
        vec![false],
        "a remotely driven step must animate, exactly as it does for the client driving it"
    );

    // A client joining later catches up to the current position instead of
    // replaying the reveals it missed.
    let latecomer = browser.open(&format!("{origin}/present")).await.expect("open");
    assert!(
        latecomer
            .wait_for(
                "window.deckShell?.slide?.id === 'overview' && window.deckShell.step === 1",
                Duration::from_secs(20),
            )
            .await
            .expect("evaluate"),
        "a client joining later did not catch up to the current slide and step"
    );

    latecomer.close().await;
    follower.close().await;
    driver.close().await;
    browser.close().await;
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
        eprintln!("skipping: no Chromium available");
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
        eprintln!("skipping: no Chromium available");
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
        eprintln!("skipping: no Chromium available");
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
    std::fs::write(&slide_path, source.replace("plain HTML", "edited HTML")).expect("write slide");

    let reloaded = page
        .wait_for(
            "window.deckShell.currentFrame()?.iframe.contentDocument\
             ?.body?.textContent?.includes('edited HTML') === true",
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
