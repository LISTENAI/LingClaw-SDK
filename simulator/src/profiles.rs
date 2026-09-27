use anyhow::{Result, ensure};
use lingclaw_sdk::capabilities::Capabilities;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

type Hotkeys = BTreeMap<String, String>;

#[derive(Clone, Serialize, Deserialize)]
pub struct CustomDevice {
    pub id: String,
    pub name: String,
    capabilities: Value,
    hotkeys: Hotkeys,
}

/// Built-in hardware is loaded from the shipped preset, never from user settings.
#[derive(Clone, Serialize, Deserialize)]
pub struct Profiles {
    active: String,
    mini_hotkeys: Hotkeys,
    custom: Vec<CustomDevice>,
}
impl Default for Profiles {
    fn default() -> Self {
        Self {
            active: "mini".into(),
            mini_hotkeys: BTreeMap::from([("function".into(), "space".into())]),
            custom: Vec::new(),
        }
    }
}
impl Profiles {
    pub fn migrate(caps: Value, keys: Hotkeys) -> Result<Self> {
        let caps = Capabilities::parse(&caps.to_string())?;
        valid_hotkeys(&caps, &keys)?;
        let mut result = Self::default();
        let mini = Capabilities::mini();
        if caps.0["hardware"] == mini.0["hardware"] && caps.0["runtime"] == mini.0["runtime"] {
            result.mini_hotkeys = keys;
        } else {
            result.clone_active();
            result.rename("自定义设备")?;
            result.update(caps)?;
            result.set_hotkeys(keys)?;
        }
        Ok(result)
    }
    pub fn validate(&self) -> Result<()> {
        valid_hotkeys(&Capabilities::mini(), &self.mini_hotkeys)?;
        let mut ids = HashSet::from(["mini"]);
        for p in &self.custom {
            ensure!(!p.id.is_empty() && ids.insert(&p.id), "设备标识重复或为空");
            ensure!(!p.name.trim().is_empty(), "设备名称不能为空");
            valid_hotkeys(
                &Capabilities::parse(&p.capabilities.to_string())?,
                &p.hotkeys,
            )?;
        }
        ensure!(ids.contains(self.active.as_str()), "所选设备不存在");
        Ok(())
    }
    pub fn active_id(&self) -> &str {
        &self.active
    }
    pub fn builtin(&self) -> bool {
        self.active == "mini"
    }
    pub fn custom(&self) -> &[CustomDevice] {
        &self.custom
    }
    pub fn name(&self) -> &str {
        self.current().map(|p| p.name.as_str()).unwrap_or("Mini")
    }
    fn current(&self) -> Option<&CustomDevice> {
        self.custom.iter().find(|p| p.id == self.active)
    }
    pub fn capabilities(&self) -> Capabilities {
        self.current()
            .map(|p| Capabilities(p.capabilities.clone()))
            .unwrap_or_else(Capabilities::mini)
    }
    pub fn hotkeys(&self) -> Hotkeys {
        self.current()
            .map(|p| &p.hotkeys)
            .unwrap_or(&self.mini_hotkeys)
            .clone()
    }
    pub fn set_hotkeys(&mut self, keys: Hotkeys) -> Result<()> {
        valid_hotkeys(&self.capabilities(), &keys)?;
        if let Some(p) = self.custom.iter_mut().find(|p| p.id == self.active) {
            p.hotkeys = keys;
        } else {
            self.mini_hotkeys = keys;
        }
        Ok(())
    }
    pub fn select(&mut self, id: &str) -> Result<()> {
        ensure!(
            id == "mini" || self.custom.iter().any(|p| p.id == id),
            "设备不存在"
        );
        self.active = id.into();
        Ok(())
    }
    pub fn clone_active(&mut self) {
        let n = (1..)
            .find(|n| !self.custom.iter().any(|p| p.id == format!("custom-{n}")))
            .unwrap();
        let name = format!("{} 副本 {n}", self.name());
        let p = CustomDevice {
            id: format!("custom-{n}"),
            name,
            capabilities: self.capabilities().0,
            hotkeys: self.hotkeys(),
        };
        self.active = p.id.clone();
        self.custom.push(p);
    }
    pub fn remove_active(&mut self) -> Result<()> {
        ensure!(!self.builtin(), "无法删除预设");
        self.custom.retain(|p| p.id != self.active);
        self.active = "mini".into();
        Ok(())
    }
    pub fn rename(&mut self, name: &str) -> Result<()> {
        ensure!(
            !name.trim().is_empty() && name.chars().count() <= 48,
            "设备名称需为 1–48 个字符"
        );
        let p = self
            .custom
            .iter_mut()
            .find(|p| p.id == self.active)
            .ok_or_else(|| anyhow::anyhow!("请先复制此配置"))?;
        p.name = name.trim().into();
        Ok(())
    }
    pub fn update(&mut self, caps: Capabilities) -> Result<()> {
        let caps = Capabilities::parse(&caps.0.to_string())?;
        let p = self
            .custom
            .iter_mut()
            .find(|p| p.id == self.active)
            .ok_or_else(|| anyhow::anyhow!("请先复制此配置"))?;
        p.hotkeys
            .retain(|id, _| caps.items("buttons").iter().any(|b| b["id"] == *id));
        p.capabilities = caps.0;
        Ok(())
    }
}
fn valid_hotkeys(c: &Capabilities, keys: &Hotkeys) -> Result<()> {
    let mut used = HashSet::new();
    for (id, key) in keys {
        ensure!(
            !key.is_empty()
                && used.insert(key)
                && c.items("buttons").iter().any(|b| b["id"] == *id),
            "热键重复、为空或按键 ID 不存在"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn builtin_is_immutable_but_mapping_is_editable() {
        let mut p = Profiles::default();
        assert!(p.update(Capabilities::mini()).is_err());
        assert!(p.rename("changed").is_err());
        assert!(p.remove_active().is_err());
        let mut legacy = Capabilities::mini().0;
        legacy["firmware_info"] = serde_json::json!({"type":"example", "version":"1.0.0"});
        let migrated = Profiles::migrate(legacy, p.hotkeys()).unwrap();
        assert!(migrated.builtin());
        assert!(migrated.custom().is_empty());
        p.set_hotkeys(BTreeMap::new()).unwrap();
        assert!(p.hotkeys().is_empty());
        assert_eq!(p.capabilities().0, Capabilities::mini().0);
    }
    #[test]
    fn clones_keep_hardware_and_mappings_independent_after_reload() {
        let mut p = Profiles::default();
        p.clone_active();
        let id = p.active_id().to_owned();
        let mut caps = p.capabilities();
        caps.0["hardware"]["display"]["width_px"] = 320.into();
        p.update(caps).unwrap();
        p.rename("桌面设备").unwrap();
        p.set_hotkeys(BTreeMap::from([("function".into(), "left".into())]))
            .unwrap();
        let mut p: Profiles = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        p.validate().unwrap();
        p.select("mini").unwrap();
        assert_eq!(p.capabilities().0, Capabilities::mini().0);
        assert_eq!(p.hotkeys()["function"], "space");
        p.select(&id).unwrap();
        assert_eq!(p.capabilities().width(), 320);
        assert_eq!(p.hotkeys()["function"], "left");
        assert_eq!(p.name(), "桌面设备");
        p.remove_active().unwrap();
        assert!(p.builtin());
        assert!(p.custom().is_empty());
        assert_eq!(p.hotkeys()["function"], "space");
    }
    #[test]
    fn migrate_preserves_custom_hardware_and_unbound_keys() {
        let mut caps = Capabilities::mini();
        caps.0["hardware"]["display"]["width_px"] = 320.into();
        let p = Profiles::migrate(caps.0.clone(), BTreeMap::new()).unwrap();
        assert!(!p.builtin());
        assert_eq!(p.capabilities().0, caps.0);
        assert!(p.hotkeys().is_empty());
        let p = Profiles::migrate(Capabilities::mini().0, BTreeMap::new()).unwrap();
        assert!(p.builtin());
        assert!(p.hotkeys().is_empty());
    }
    #[test]
    fn malformed_profiles_are_rejected() {
        let mut p = Profiles::default();
        p.clone_active();
        p.custom[0].id = "mini".into();
        assert!(p.validate().is_err());
        assert!(Profiles::default().select("absent").is_err());
        assert!(
            Profiles::default()
                .set_hotkeys(BTreeMap::from([("absent".into(), "a".into())]))
                .is_err()
        );
    }
}
