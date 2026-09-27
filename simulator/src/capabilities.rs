use crate::api::{LATEST_VERSION, MIN_VERSION};
use anyhow::{Result, bail};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub struct Capabilities(pub Value);
impl Capabilities {
    pub fn parse(source: &str) -> Result<Self> {
        let c = Self(serde_json::from_str(source)?);
        if c.0["schema_version"] != 2 {
            bail!("仅支持 capabilities schema 2");
        }
        let sdk = &c.0["runtime"]["lua_sdk"];
        if !sdk["version"]
            .as_u64()
            .is_some_and(|v| (MIN_VERSION..=LATEST_VERSION).contains(&v))
        {
            bail!("支持的 API 版本为 {MIN_VERSION}–{LATEST_VERSION}");
        }
        for (key, min, max) in [
            ("source_max_bytes", 1, 65536),
            ("heap_max_bytes", 65536, 524288),
        ] {
            if !sdk[key].as_u64().is_some_and(|v| (min..=max).contains(&v)) {
                bail!("{key} 超出支持范围");
            }
        }
        if c.has_screen()
            && !c.0["hardware"]["display"]["width_px"]
                .as_u64()
                .is_some_and(|v| (1..=2048).contains(&v))
        {
            bail!("屏幕宽度应为 1–2048");
        }
        if c.has_screen()
            && !c.0["hardware"]["display"]["height_px"]
                .as_u64()
                .is_some_and(|v| (1..=2048).contains(&v))
        {
            bail!("屏幕高度应为 1–2048");
        }
        for kind in ["buttons", "leds"] {
            let mut ids = HashSet::new();
            for item in c.items(kind) {
                let id = item["id"].as_str().unwrap_or("");
                if id.is_empty() || id.len() > 127 || !ids.insert(id) {
                    bail!("{kind} ID 为空、过长或重复");
                }
                if kind == "buttons"
                    && item["events"]
                        .as_array()
                        .is_none_or(|a| a.iter().any(|e| e != "click"))
                {
                    bail!("按键事件仅支持 click");
                }
            }
        }
        Ok(c)
    }
    pub fn mini() -> Self {
        let profile: Value = serde_json::from_str(include_str!("../profiles/mini.json")).unwrap();
        Self::parse(
            &serde_json::json!({
                "schema_version": 2,
                "runtime": {"lua_sdk": crate::api::defaults()},
                "hardware": profile["hardware"],
            })
            .to_string(),
        )
        .unwrap()
    }
    pub fn sdk(&self) -> &Value {
        &self.0["runtime"]["lua_sdk"]
    }
    pub fn api(&self) -> u64 {
        self.sdk()["version"].as_u64().unwrap()
    }
    pub fn has_screen(&self) -> bool {
        self.0["hardware"]["display"]["supported"] == true
    }
    pub fn width(&self) -> u32 {
        self.0["hardware"]["display"]["width_px"]
            .as_u64()
            .unwrap_or(0) as u32
    }
    pub fn height(&self) -> u32 {
        self.0["hardware"]["display"]["height_px"]
            .as_u64()
            .unwrap_or(0) as u32
    }
    pub fn items(&self, kind: &str) -> &[Value] {
        let group = if kind == "buttons" {
            "input"
        } else {
            "lighting"
        };
        self.0["hardware"][group][kind]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
    pub fn speaker(&self) -> bool {
        self.0["hardware"]["audio"]["speaker"] == true
    }
    pub fn http(&self) -> bool {
        self.api() >= 4 && self.0["hardware"]["network"] == true && self.sdk()["http"].is_object()
    }
    pub fn tts(&self) -> bool {
        self.api() >= 4
            && self.speaker()
            && self.0["hardware"]["network"] == true
            && self.sdk()["tts"].is_object()
    }
}
