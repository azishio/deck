//! `deck doctor` — environment diagnostics (design doc 17.1).

use camino::Utf8Path;
use serde::Serialize;

use crate::browser::BrowserSession;
use crate::error::Result;
use crate::lock::Lock;
use crate::project::Project;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

impl Status {
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Ok => "✓",
            Self::Warn => "⚠",
            Self::Fail => "✗",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub name: String,
    pub status: Status,
    pub detail: String,
}

impl Check {
    fn new(name: &str, status: Status, detail: impl Into<String>) -> Self {
        Self { name: name.to_owned(), status, detail: detail.into() }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub checks: Vec<Check>,
}

impl DoctorReport {
    pub fn failed(&self) -> bool {
        self.checks.iter().any(|check| check.status == Status::Fail)
    }

    pub fn to_text(&self) -> String {
        self.checks
            .iter()
            .map(|check| format!("{} {:<24} {}", check.status.symbol(), check.name, check.detail))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub async fn run(project: &Project) -> Result<DoctorReport> {
    let config = project.config();
    let mut checks = Vec::new();

    // Chromium executable ------------------------------------------------
    match locate_executable(&config.browser.command) {
        Some(path) => checks.push(Check::new("chromium executable", Status::Ok, path)),
        None => checks.push(Check::new(
            "chromium executable",
            Status::Fail,
            format!(
                "{} が見つかりません。deck.local.toml の [browser] command で指定してください",
                config.browser.command
            ),
        )),
    }

    // Launch + CDP -------------------------------------------------------
    match BrowserSession::launch(&config.browser, config.canvas).await {
        Ok(session) => {
            match session.version().await {
                Ok(version) => checks.push(Check::new("chromium version", Status::Ok, version)),
                Err(error) => {
                    checks.push(Check::new("chromium version", Status::Warn, error.to_string()))
                }
            }
            match session.open("about:blank").await {
                Ok(page) => {
                    checks.push(Check::new("cdp connection", Status::Ok, "接続できました"));
                    checks.push(font_check(&page, project).await);
                    page.close().await;
                }
                Err(error) => {
                    checks.push(Check::new("cdp connection", Status::Fail, error.to_string()))
                }
            }
            session.close().await;
        }
        Err(error) => checks.push(Check::new("chromium launch", Status::Fail, error.to_string())),
    }

    // Writable directories ----------------------------------------------
    for directory in [project.work_dir(), project.root().join(&config.build.output_dir)] {
        checks.push(match writable(&directory) {
            Ok(()) => Check::new("writable directory", Status::Ok, directory.to_string()),
            Err(error) => {
                Check::new("writable directory", Status::Fail, format!("{directory}: {error}"))
            }
        });
    }

    // Port ---------------------------------------------------------------
    let port = config.server.port;
    checks.push(match tokio::net::TcpListener::bind((config.server.host.as_str(), port)).await {
        Ok(listener) => {
            let detail = listener
                .local_addr()
                .map(|addr| addr.to_string())
                .unwrap_or_else(|_| format!("{}:{port}", config.server.host));
            Check::new("port", Status::Ok, detail)
        }
        Err(error) => {
            Check::new("port", Status::Fail, format!("{}:{port}: {error}", config.server.host))
        }
    });

    // deck.lock ----------------------------------------------------------
    checks.push(match Lock::load(project.root())? {
        None => Check::new("deck.lock", Status::Warn, "存在しません (`deck build` で生成されます)"),
        Some(lock) => {
            let drift = lock.drift();
            if drift.is_empty() {
                Check::new("deck.lock", Status::Ok, "一致しています")
            } else {
                Check::new("deck.lock", Status::Warn, drift.join(" / "))
            }
        }
    });

    Ok(DoctorReport { checks })
}

async fn font_check(page: &crate::browser::PageSession, project: &Project) -> Check {
    let families = font_families(project);
    if families.is_empty() {
        return Check::new("fonts", Status::Ok, "確認するフォント指定がありません");
    }

    let script = format!(
        "JSON.stringify({}.filter((family) => !document.fonts.check(`16px \"${{family}}\"`)))",
        serde_json::to_string(&families).unwrap_or_else(|_| "[]".into())
    );
    match page.evaluate::<String>(&script).await {
        Ok(json) => match serde_json::from_str::<Vec<String>>(&json) {
            Ok(missing) if missing.is_empty() => Check::new(
                "fonts",
                Status::Ok,
                format!("{} 件のフォントを確認しました", families.len()),
            ),
            Ok(missing) => {
                Check::new("fonts", Status::Warn, format!("未インストール: {}", missing.join(", ")))
            }
            Err(error) => Check::new("fonts", Status::Warn, error.to_string()),
        },
        Err(error) => Check::new("fonts", Status::Warn, error.to_string()),
    }
}

/// Font families named by `--deck-font-*` declarations in the project styles.
fn font_families(project: &Project) -> Vec<String> {
    const GENERIC: [&str; 6] =
        ["serif", "sans-serif", "monospace", "cursive", "fantasy", "system-ui"];

    let mut families = Vec::new();
    let sources = crate::assets::design_css(project).unwrap_or_default();
    for line in sources.lines() {
        let Some((name, value)) = line.split_once(':') else { continue };
        if !name.trim().starts_with("--deck-font-") || name.contains("size") {
            continue;
        }
        for family in value.trim_end_matches([';', ' ']).split(',') {
            let family = family.trim().trim_matches(['"', '\'']).to_owned();
            if family.is_empty()
                || GENERIC.contains(&family.as_str())
                || family.starts_with("ui-")
                || families.contains(&family)
            {
                continue;
            }
            families.push(family);
        }
    }
    families
}

fn writable(directory: &Utf8Path) -> std::io::Result<()> {
    std::fs::create_dir_all(directory)?;
    let probe = directory.join(".deck-write-probe");
    std::fs::write(&probe, b"")?;
    std::fs::remove_file(&probe)
}

/// Resolve a command through `PATH`, or accept an absolute path.
fn locate_executable(command: &str) -> Option<String> {
    let candidate = Utf8Path::new(command);
    if candidate.is_absolute() {
        return candidate.is_file().then(|| command.to_owned());
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(command))
        .find(|candidate| candidate.is_file())
        .map(|found| found.display().to_string())
}
