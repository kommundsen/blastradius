//! Library facade for the CLI crate: exposes the MCP server so integration
//! tests can drive the protocol without spawning a process.
pub mod mcp;
pub mod onboard;
