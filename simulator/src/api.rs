//! Public API versions and default runtime limits, independent of device presets.
use serde_json::Value;

pub const MIN_VERSION: u64 = 2;
pub const LATEST_VERSION: u64 = 4;

pub fn defaults() -> Value {
    let mut sdk: Value = serde_json::from_str(include_str!("api-defaults.json")).unwrap();
    sdk["version"] = LATEST_VERSION.into();
    sdk
}
