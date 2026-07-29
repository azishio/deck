//! Core of `deck`: a local slide runtime where one slide is one HTML document.
//!
//! Module layout follows the design document (18. Rust Architecture):
//! configuration, slide discovery, the internal manifest, the design system
//! assets, the Axum server, the file watcher, headless Chromium checks, report
//! rendering and the static build.

pub mod assets;
pub mod browser;
pub mod build;
pub mod check;
pub mod config;
pub mod discovery;
pub mod doctor;
pub mod error;
pub mod lock;
pub mod manifest;
pub mod project;
pub mod render;
pub mod report;
pub mod scaffold;
pub mod server;
pub mod watcher;

pub use error::{Error, Result};
pub use project::Project;
