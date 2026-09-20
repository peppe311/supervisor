//! A client of the official Codex runtime, not a model or tool execution engine.
//! The application retains presentation/ownership; App Server owns agent work.

/// Stable identity used for App Server compliance attribution and thread service
/// metadata. Changing this requires coordinating the registered client name with
/// OpenAI before an enterprise release.
pub const CLIENT_NAME: &str = "central_agent";

pub mod api;
pub mod cloud;
pub mod configuration;
pub mod conversations;
pub mod goals;
pub mod mcp_configuration;
pub mod mirror;
pub mod model_notices;
pub mod requests;
pub mod runtime;
pub mod thread_items;
pub mod thread_metadata;
pub mod thread_sections;
pub mod thread_settings;
pub mod transport;
pub mod wire;

#[cfg(windows)]
mod windows_process;
