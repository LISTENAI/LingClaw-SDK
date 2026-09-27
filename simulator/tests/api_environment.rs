use lingclaw_sdk::{api, capabilities::Capabilities};

#[test]
fn preset_combines_hardware_with_current_api_environment() {
    let profile: serde_json::Value =
        serde_json::from_str(include_str!("../profiles/mini.json")).unwrap();
    assert!(profile.get("runtime").is_none());
    assert!(profile.get("firmware_info").is_none());
    let caps = Capabilities::mini();
    assert_eq!(caps.api(), api::LATEST_VERSION);
    assert_eq!(caps.0["hardware"], profile["hardware"]);
}

#[test]
fn firmware_metadata_does_not_select_api_version() {
    let mut caps = Capabilities::mini();
    caps.0["firmware_info"] = serde_json::json!({"type":"example", "version":"999.0.0"});
    caps.0["runtime"]["lua_sdk"]["version"] = api::MIN_VERSION.into();
    assert_eq!(
        Capabilities::parse(&caps.0.to_string()).unwrap().api(),
        api::MIN_VERSION
    );
    caps.0["runtime"]["lua_sdk"]["version"] = (api::LATEST_VERSION + 1).into();
    assert!(Capabilities::parse(&caps.0.to_string()).is_err());
}
