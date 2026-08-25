//! Library facade for the CLI crate: exposes the MCP server so integration
//! tests can drive the protocol without spawning a process.
pub mod mcp;

// The format reference and agent onboarding live in core: the desktop app
// needs both (it offers to initialise a folder and register the agents) and
// it cannot depend on this crate.
pub use blastradius_core::{format_ref, onboard};
