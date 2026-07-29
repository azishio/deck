use std::fmt;

/// Result alias used throughout `deck-core`.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Errors surfaced by the CLI. The variant determines the process exit code
/// (design doc 17.3).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Configuration or input problem. Exit code 2.
    #[error("{0}")]
    Config(String),

    /// Browser launch or connection problem. Exit code 3.
    #[error("{0}")]
    Browser(String),

    /// Render or build problem. Exit code 4.
    #[error("{0}")]
    Render(String),

    /// Checks reported violations. Exit code 1.
    #[error("checks reported {errors} error(s) and {warnings} warning(s)")]
    CheckViolations { errors: usize, warnings: usize },

    #[error("{path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

impl Error {
    pub fn config(message: impl fmt::Display) -> Self {
        Self::Config(message.to_string())
    }

    pub fn browser(message: impl fmt::Display) -> Self {
        Self::Browser(message.to_string())
    }

    pub fn render(message: impl fmt::Display) -> Self {
        Self::Render(message.to_string())
    }

    pub fn io(path: impl fmt::Display, source: std::io::Error) -> Self {
        Self::Io { path: path.to_string(), source }
    }

    /// Process exit code for this error.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::CheckViolations { .. } => 1,
            Self::Config(_) | Self::Io { .. } => 2,
            Self::Browser(_) => 3,
            Self::Render(_) => 4,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(source: std::io::Error) -> Self {
        Self::Io { path: "I/O".into(), source }
    }
}

/// Read a file, attaching its path to any I/O error.
pub(crate) fn read_to_string(path: &camino::Utf8Path) -> Result<String> {
    std::fs::read_to_string(path).map_err(|source| Error::io(path, source))
}

/// Write a file, creating parent directories, attaching the path to errors.
pub(crate) fn write_file(path: &camino::Utf8Path, contents: impl AsRef<[u8]>) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::io(parent, source))?;
    }
    std::fs::write(path, contents).map_err(|source| Error::io(path, source))
}
