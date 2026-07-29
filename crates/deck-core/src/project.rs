//! Project root discovery and the fixed directory layout (design doc 5, 6.2).

use camino::{Utf8Path, Utf8PathBuf};

use crate::config::{
    ASSETS_DIR, COMPONENTS_DIR, CONFIG_FILE, Config, DESIGN_DIR, Overrides, SLIDES_DIR, WORK_DIR,
};
use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct Project {
    root: Utf8PathBuf,
    config: Config,
    config_path: Utf8PathBuf,
}

impl Project {
    /// Locate the project root and load its configuration.
    ///
    /// `root` is used verbatim when given; otherwise the current directory and
    /// its ancestors are searched for `deck.toml`.
    pub fn open(
        root: Option<&Utf8Path>,
        config_path: Option<&Utf8Path>,
        overrides: &Overrides,
    ) -> Result<Self> {
        let root = match root {
            Some(path) => absolute(path)?,
            None => discover_root(config_path)?,
        };
        let config_path = config_path.map_or_else(|| root.join(CONFIG_FILE), Utf8Path::to_path_buf);
        let config = Config::load(&root, Some(&config_path), overrides)?;
        Ok(Self { root, config, config_path })
    }

    /// Build a project handle without requiring `deck.toml` to exist.
    pub fn with_config(root: Utf8PathBuf, config: Config) -> Self {
        let config_path = root.join(CONFIG_FILE);
        Self { root, config, config_path }
    }

    pub fn root(&self) -> &Utf8Path {
        &self.root
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn config_path(&self) -> &Utf8Path {
        &self.config_path
    }

    pub fn slides_dir(&self) -> Utf8PathBuf {
        self.root.join(SLIDES_DIR)
    }

    pub fn components_dir(&self) -> Utf8PathBuf {
        self.root.join(COMPONENTS_DIR)
    }

    pub fn design_dir(&self) -> Utf8PathBuf {
        self.root.join(DESIGN_DIR)
    }

    pub fn assets_dir(&self) -> Utf8PathBuf {
        self.root.join(ASSETS_DIR)
    }

    pub fn work_dir(&self) -> Utf8PathBuf {
        self.root.join(WORK_DIR)
    }

    pub fn components_entry(&self) -> Utf8PathBuf {
        self.root.join(&self.config.components.entry)
    }

    /// Directories watched for hot reload (design doc 11.8).
    pub fn watched_dirs(&self) -> Vec<Utf8PathBuf> {
        vec![self.slides_dir(), self.components_dir(), self.design_dir(), self.assets_dir()]
    }

    /// Path relative to the project root, using `/` separators.
    pub fn relative(&self, path: &Utf8Path) -> Option<String> {
        path.strip_prefix(&self.root).ok().map(|relative| relative.as_str().replace('\\', "/"))
    }
}

fn absolute(path: &Utf8Path) -> Result<Utf8PathBuf> {
    let absolute = if path.is_absolute() { path.to_path_buf() } else { current_dir()?.join(path) };
    Ok(normalize(&absolute))
}

/// Resolve `.`/`..` lexically; the path need not exist yet.
fn normalize(path: &Utf8Path) -> Utf8PathBuf {
    let mut out = Utf8PathBuf::new();
    for component in path.components() {
        match component {
            camino::Utf8Component::ParentDir => {
                out.pop();
            }
            camino::Utf8Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

fn current_dir() -> Result<Utf8PathBuf> {
    let cwd = std::env::current_dir().map_err(|error| Error::io("current directory", error))?;
    Utf8PathBuf::from_path_buf(cwd)
        .map_err(|path| Error::config(format!("UTF-8でないパスです: {}", path.display())))
}

fn discover_root(config_path: Option<&Utf8Path>) -> Result<Utf8PathBuf> {
    if let Some(path) = config_path {
        let path = absolute(path)?;
        return path
            .parent()
            .map(Utf8Path::to_path_buf)
            .ok_or_else(|| Error::config(format!("--config のパスが不正です: {path}")));
    }

    let start = current_dir()?;
    for candidate in start.ancestors() {
        if candidate.join(CONFIG_FILE).is_file() {
            return Ok(candidate.to_path_buf());
        }
    }
    Err(Error::config(format!(
        "{CONFIG_FILE} が見つかりません ({start} とその親ディレクトリを探索しました)。`deck init` で作成できます"
    )))
}
