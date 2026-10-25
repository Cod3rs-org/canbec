//! canbec application layer: use cases ([`BrowserService`]) and the ports they need.
//! Driving adapters (sidebar UI, later MCP) call the service; driven adapters
//! (engines, storage) implement the ports.

mod browser;
pub mod ports;
pub mod ui;

pub use browser::{resolve_input, BrowserService, Snapshot, HOME_URL};
