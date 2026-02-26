mod request;
mod response;
mod notification;
mod types;

pub const LATEST_PROTOCOL_VERSION: &str = "2024-11-05";
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &[LATEST_PROTOCOL_VERSION, "2024-10-07"];
pub const JSONRPC_VERSION: &str = "2.0";

pub use request::*;
pub use response::*;
pub use notification::*;
pub use types::*;