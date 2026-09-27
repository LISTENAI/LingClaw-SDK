use crate::{
    capabilities::Capabilities,
    render::{Rect, Scene, Text},
};
use chrono::{Datelike, Local, Timelike};
use mlua::{HookTriggers, Lua, LuaOptions, LuaSerdeExt, StdLib, Table, Value, VmState};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    rc::Rc,
    time::Instant,
};

type Result<T> = mlua::Result<T>;
fn error(s: impl Into<String>) -> mlua::Error {
    mlua::Error::RuntimeError(s.into())
}
fn range(v: i64, min: i64, max: i64) -> Result<i64> {
    if v < min || v > max {
        Err(error("argument out of range"))
    } else {
        Ok(v)
    }
}
fn limit(c: &Capabilities, group: &str, key: &str, default: u64) -> u64 {
    c.sdk()[group][key].as_u64().unwrap_or(default)
}
#[derive(Clone, Debug)]
pub enum Led {
    Off,
    On,
    Blink(u64, u64, u64),
}
#[derive(Clone, Serialize, Deserialize)]
struct Save {
    data: Json,
    expires: i64,
    written: i64,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Store {
    entries: BTreeMap<String, Save>,
    #[serde(skip)]
    pub path: Option<PathBuf>,
}
impl Store {
    pub fn open(path: PathBuf) -> Self {
        let mut store: Self = std::fs::read(&path)
            .ok()
            .and_then(|s| serde_json::from_slice(&s).ok())
            .unwrap_or_default();
        store.path = Some(path);
        store
    }
    fn flush(&self) -> std::io::Result<()> {
        if let Some(path) = &self.path {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            // One desktop process owns this store. Write a complete file before replacement.
            let tmp = path.with_extension("tmp");
            std::fs::write(&tmp, serde_json::to_vec(self)?)?;
            #[cfg(target_os = "windows")]
            if path.exists() {
                std::fs::remove_file(path)?;
            }
            std::fs::rename(tmp, path)?;
        }
        Ok(())
    }
}
struct Pending {
    id: u64,
    kind: &'static str,
    created: u64,
    result: Option<Json>,
    http: Option<crate::network::Transfer>,
}
#[derive(Clone, Copy, Debug)]
pub struct Tone {
    pub hz: u32,
    pub ms: u32,
}
pub struct State {
    pub tones: VecDeque<Tone>,
    pub scene: Scene,
    draft: Scene,
    pub frames: u64,
    pub elapsed: u64,
    pub leds: BTreeMap<String, Led>,
    pub log: VecDeque<String>,
    pub clock_synced: bool,
    pub speech: Option<(u64, String)>,
    pub http_fixture: Option<Json>,
    pending: Vec<Pending>,
    next_id: u64,
    store: Rc<RefCell<Store>>,
    id: String,
    read_storage: bool,
    staging: bool,
    exiting: bool,
    dirty: bool,
}
impl State {
    fn log(&mut self, s: String) {
        self.log.push_back(s);
        while self.log.len() > 100 {
            self.log.pop_front();
        }
    }
}
pub struct Runtime {
    lua: Lua,
    pub state: Rc<RefCell<State>>,
    pub caps: Capabilities,
    budget: Rc<Cell<i64>>,
    deadline: Rc<Cell<Instant>>,
    wall_limit: Rc<Cell<u128>>,
    pub stopped: bool,
    pub fault: Option<String>,
}
impl Runtime {
    pub fn new(
        source: &str,
        id: &str,
        caps: Capabilities,
        store: Rc<RefCell<Store>>,
    ) -> Result<Self> {
        Self::create(source, id, caps, store, None)
    }
    pub fn with_fixture(
        source: &str,
        id: &str,
        caps: Capabilities,
        store: Rc<RefCell<Store>>,
        fixture: Json,
    ) -> Result<Self> {
        Self::create(source, id, caps, store, Some(fixture))
    }
    fn create(
        source: &str,
        id: &str,
        caps: Capabilities,
        store: Rc<RefCell<Store>>,
        fixture: Option<Json>,
    ) -> Result<Self> {
        if source.is_empty()
            || source.len() > caps.sdk()["source_max_bytes"].as_u64().unwrap() as usize
            || source.contains('\0')
        {
            return Err(error("invalid Lua source size or NUL byte"));
        }
        let lua = Lua::new_with(
            StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8,
            LuaOptions::default(),
        )?;
        lua.set_memory_limit(caps.sdk()["heap_max_bytes"].as_u64().unwrap() as usize)?;
        let candidate_store = Rc::new(RefCell::new(store.borrow().clone()));
        let state = Rc::new(RefCell::new(State {
            scene: Scene::default(),
            draft: Scene::default(),
            frames: 0,
            elapsed: 0,
            leds: caps
                .items("leds")
                .iter()
                .map(|v| (v["id"].as_str().unwrap().to_owned(), Led::Off))
                .collect(),
            log: VecDeque::new(),
            clock_synced: true,
            speech: None,
            http_fixture: fixture,
            pending: vec![],
            tones: VecDeque::new(),
            next_id: 1,
            store: candidate_store,
            id: id.into(),
            read_storage: false,
            staging: true,
            exiting: false,
            dirty: false,
        }));
        let budget = Rc::new(Cell::new(500_000i64));
        let deadline = Rc::new(Cell::new(Instant::now()));
        let wall_limit = Rc::new(Cell::new(150u128));
        let wall = wall_limit.clone();
        let b = budget.clone();
        let d = deadline.clone();
        lua.set_hook(
            HookTriggers::new().every_nth_instruction(1000),
            move |_, _| {
                b.set(b.get() - 1000);
                if b.get() <= 0 {
                    return Err(error("instruction budget exceeded"));
                }
                if d.get().elapsed().as_millis() > wall.get() {
                    return Err(error("callback time budget exceeded"));
                }
                Ok(VmState::Continue)
            },
        )?;
        let mut runtime = Self {
            lua,
            state,
            caps,
            budget,
            deadline,
            wall_limit,
            stopped: false,
            fault: None,
        };
        runtime.register()?;
        runtime.deadline.set(Instant::now());
        let startup = (|| -> Result<()> {
            runtime
                .lua
                .load(source)
                .set_name("@lingclaw")
                .set_mode(mlua::ChunkMode::Text)
                .exec()?;
            if !matches!(
                runtime.lua.globals().get::<Value>("on_tick")?,
                Value::Function(_)
            ) {
                return Err(error("on_tick must be defined"));
            }
            runtime.invoke("on_start", ())?;
            runtime.invoke("on_tick", 20)?;
            if runtime.caps.has_screen() && runtime.state.borrow().frames == 0 {
                return Err(error("startup must present a frame"));
            }
            Ok(())
        })();
        if let Err(e) = startup {
            if runtime.state.borrow().read_storage {
                let mut store = store.borrow_mut();
                if let Some(entry) = store.entries.get_mut(id) {
                    entry.expires = 0;
                }
                let _ = store.flush();
            }
            return Err(e);
        }
        {
            let mut s = runtime.state.borrow_mut();
            if s.dirty {
                *store.borrow_mut() = s.store.borrow().clone();
                if let Err(e) = store.borrow().flush() {
                    s.log(format!("storage: {e}"));
                }
            }
            s.store = store;
            s.staging = false;
        }
        Ok(runtime)
    }
    fn register(&mut self) -> Result<()> {
        let lua = &self.lua;
        let globals = lua.globals();
        for name in [
            "collectgarbage",
            "dofile",
            "load",
            "loadfile",
            "print",
            "warn",
            "pcall",
            "xpcall",
            "setmetatable",
        ] {
            globals.set(name, Value::Nil)?;
        }
        let app = lua.create_table()?;
        app.set("api_version", self.caps.api())?;
        if self.caps.has_screen() {
            app.set("width", self.caps.width())?;
            app.set("height", self.caps.height())?;
        }
        globals.set("app", app)?;
        if self.caps.has_screen() {
            let screen = lua.create_table()?;
            let s = self.state.clone();
            screen.set(
                "begin",
                lua.create_function(move |_, color: i64| {
                    let mut s = s.borrow_mut();
                    s.draft = Scene {
                        background: color as u32 & 0xffffff,
                        ..Default::default()
                    };
                    Ok(())
                })?,
            )?;
            let s = self.state.clone();
            let (w, h) = (self.caps.width() as i64, self.caps.height() as i64);
            screen.set(
                "rect",
                lua.create_function(move |_, (x, y, rw, rh, c): (i64, i64, i64, i64, i64)| {
                    range(x, 0, w - 1)?;
                    range(y, 0, h - 1)?;
                    range(rw, 1, w - x)?;
                    range(rh, 1, h - y)?;
                    let mut s = s.borrow_mut();
                    if s.draft.rects.len() >= 128 {
                        return Err(error("screen.rect limit is 128"));
                    }
                    s.draft.rects.push(Rect {
                        x: x as i32,
                        y: y as i32,
                        w: rw as i32,
                        h: rh as i32,
                        color: c as u32 & 0xffffff,
                    });
                    Ok(())
                })?,
            )?;
            let s = self.state.clone();
            screen.set(
                "text",
                lua.create_function(move |_, (text, x, y, c): (mlua::String, i64, i64, i64)| {
                    range(x, 0, w - 1)?;
                    range(y, 0, h - 16)?;
                    let bytes = text.as_bytes();
                    if bytes.len() > 63 {
                        return Err(error("text is too long (63 UTF-8 bytes)"));
                    }
                    let mut s = s.borrow_mut();
                    if s.draft.texts.len() >= 8 {
                        return Err(error("screen.text limit is 8"));
                    }
                    let mut t = Text {
                        x: x as i32,
                        y: y as i32,
                        color: c as u32 & 0xffffff,
                        text: [0; 64],
                    };
                    t.text[..bytes.len()].copy_from_slice(&bytes);
                    s.draft.texts.push(t);
                    Ok(())
                })?,
            )?;
            let s = self.state.clone();
            screen.set(
                "present",
                lua.create_function(move |_, ()| {
                    let mut s = s.borrow_mut();
                    s.scene = s.draft.clone();
                    s.frames += 1;
                    Ok(())
                })?,
            )?;
            globals.set("screen", screen)?;
        }
        if !self.caps.items("leds").is_empty() {
            let led = lua.create_table()?;
            for (name, on) in [("on", true), ("off", false)] {
                let s = self.state.clone();
                led.set(
                    name,
                    lua.create_function(move |_, id: String| {
                        let mut s = s.borrow_mut();
                        let target = s.leds.get_mut(&id).ok_or_else(|| error("unknown LED id"))?;
                        *target = if on { Led::On } else { Led::Off };
                        Ok(())
                    })?,
                )?;
            }
            let s = self.state.clone();
            led.set(
                "blink",
                lua.create_function(move |_, (id, on, off): (String, i64, i64)| {
                    range(on, 10, 5000)?;
                    range(off, 10, 5000)?;
                    let mut s = s.borrow_mut();
                    let elapsed = s.elapsed;
                    *s.leds.get_mut(&id).ok_or_else(|| error("unknown LED id"))? =
                        Led::Blink(on as u64, off as u64, elapsed);
                    Ok(())
                })?,
            )?;
            globals.set("led", led)?;
        }
        if self.caps.speaker() {
            let buzzer = lua.create_table()?;
            let s = self.state.clone();
            buzzer.set(
                "play",
                lua.create_function(move |_, (hz, ms): (i64, i64)| {
                    range(hz, 100, 5000)?;
                    range(ms, 20, 3000)?;
                    let mut state = s.borrow_mut();
                    if state.tones.len() < 4 && state.speech.is_none() {
                        state.tones.push_back(Tone {
                            hz: hz as u32,
                            ms: ms as u32,
                        });
                    }
                    Ok(())
                })?,
            )?;
            globals.set("buzzer", buzzer)?;
        }
        if self.caps.api() >= 3 {
            self.register_clock_storage()?;
        }
        if self.caps.api() >= 4 {
            crate::json_api::register(
                lua,
                self.caps.sdk()["heap_max_bytes"].as_u64().unwrap() as usize,
            )?;
        }
        if self.caps.http() {
            self.register_http()?;
        }
        if self.caps.tts() {
            self.register_tts()?;
        }
        Ok(())
    }
    fn register_clock_storage(&self) -> Result<()> {
        let lua = &self.lua;
        let clock = lua.create_table()?;
        let s = self.state.clone();
        clock.set(
            "now",
            lua.create_function(move |_, ()| {
                Ok(s.borrow()
                    .clock_synced
                    .then(|| chrono::Utc::now().timestamp()))
            })?,
        )?;
        let s = self.state.clone();
        clock.set(
            "localtime",
            lua.create_function(move |lua, ()| {
                if !s.borrow().clock_synced {
                    return Ok(Value::Nil);
                }
                let d = Local::now();
                let t = lua.create_table()?;
                for (k, v) in [
                    ("year", d.year()),
                    ("month", d.month() as i32),
                    ("day", d.day() as i32),
                    ("hour", d.hour() as i32),
                    ("min", d.minute() as i32),
                    ("sec", d.second() as i32),
                    ("wday", d.weekday().num_days_from_sunday() as i32),
                    ("utc_offset", d.offset().local_minus_utc()),
                ] {
                    t.set(k, v)?;
                }
                Ok(Value::Table(t))
            })?,
        )?;
        lua.globals().set("clock", clock)?;
        let storage = lua.create_table()?;
        let s = self.state.clone();
        storage.set(
            "load",
            lua.create_function(move |lua, ()| {
                let mut s = s.borrow_mut();
                s.read_storage = true;
                if !s.clock_synced {
                    return Ok(Value::Nil);
                }
                let store = s.store.borrow();
                let value = store
                    .entries
                    .get(&s.id)
                    .filter(|e| e.expires > chrono::Utc::now().timestamp())
                    .map(|e| &e.data);
                match value {
                    Some(v) => lua.to_value(v),
                    None => Ok(Value::Nil),
                }
            })?,
        )?;
        let s = self.state.clone();
        let caps = self.caps.clone();
        storage.set(
            "save",
            lua.create_function(move |lua, (table, ttl): (Table, Option<i64>)| {
                let mut data = serde_json::Map::new();
                for pair in table.pairs::<Value, Value>() {
                    let (k, v) = pair?;
                    let Value::String(key) = k else {
                        return Ok((false, Some("invalid_data".to_owned())));
                    };
                    let key = key.to_str()?.to_owned();
                    if key.is_empty() || key.len() > 32 || key.contains('\0') {
                        return Ok((false, Some("invalid_data".into())));
                    }
                    let valid = match &v {
                        Value::Integer(_) | Value::Boolean(_) => true,
                        Value::Number(n) => n.is_finite(),
                        Value::String(s) => s.as_bytes().len() <= 256 && !s.as_bytes().contains(&0),
                        _ => false,
                    };
                    if !valid {
                        return Ok((false, Some("invalid_data".into())));
                    }
                    data.insert(key, lua.from_value::<Json>(v)?);
                }
                if data.len() > 32 {
                    return Ok((false, Some("invalid_data".into())));
                }
                let data = Json::Object(data);
                if serde_json::to_vec(&data).unwrap().len()
                    > limit(&caps, "storage", "max_bytes", 1024) as usize
                {
                    return Ok((false, Some("too_large".into())));
                }
                let ttl =
                    ttl.unwrap_or(limit(&caps, "storage", "default_ttl_seconds", 604800) as i64);
                if ttl < 1 || ttl > limit(&caps, "storage", "max_ttl_seconds", 2592000) as i64 {
                    return Ok((false, Some("invalid_ttl".into())));
                }
                let mut s = s.borrow_mut();
                if !s.clock_synced {
                    return Ok((false, Some("clock_unavailable".into())));
                }
                let now = chrono::Utc::now().timestamp_millis();
                {
                    let mut store = s.store.borrow_mut();
                    if store.entries.get(&s.id).is_some_and(|e| {
                        now - e.written < limit(&caps, "storage", "write_interval_ms", 10000) as i64
                    }) {
                        return Ok((false, Some("rate_limited".into())));
                    }
                    store.entries.retain(|_, e| {
                        e.expires > now / 1000
                            || now - e.written
                                < limit(&caps, "storage", "write_interval_ms", 10000) as i64
                    });
                    if !store.entries.contains_key(&s.id)
                        && store.entries.len() >= limit(&caps, "storage", "max_apps", 4) as usize
                    {
                        return Ok((false, Some("storage_full".into())));
                    }
                    store.entries.insert(
                        s.id.clone(),
                        Save {
                            data,
                            expires: now / 1000 + ttl,
                            written: now,
                        },
                    );
                    if !s.staging && store.flush().is_err() {
                        return Ok((false, Some("io_error".into())));
                    }
                }
                s.dirty = true;
                Ok((true, None))
            })?,
        )?;
        let s = self.state.clone();
        storage.set(
            "clear",
            lua.create_function(move |_, ()| {
                let mut s = s.borrow_mut();
                {
                    let mut store = s.store.borrow_mut();
                    if let Some(e) = store.entries.get_mut(&s.id) {
                        e.expires = 0;
                    }
                    if !s.staging && store.flush().is_err() {
                        return Ok((false, Some("io_error")));
                    }
                }
                s.dirty = true;
                Ok((true, None))
            })?,
        )?;
        lua.globals().set("storage", storage)?;
        Ok(())
    }
    fn register_http(&self) -> Result<()> {
        let table = self.lua.create_table()?;
        for get in [true, false] {
            let s = self.state.clone();
            let caps = self.caps.clone();
            let f = self.lua.create_function(move |lua, args: mlua::MultiValue| {
                let options = if get {
                    let table = match args.get(1) {
                        Some(Value::Table(t)) => t.clone(),
                        None | Some(Value::Nil) => lua.create_table()?,
                        _ => return Err(error("HTTP options must be a table")),
                    };
                    let copy = lua.create_table()?;
                    for pair in table.pairs::<Value, Value>() {
                        let (k, v) = pair?;
                        copy.set(k, v)?;
                    }
                    copy.set("url", args.front().cloned().unwrap_or(Value::Nil))?;
                    copy
                } else {
                    match args.front() {
                        Some(Value::Table(t)) => t.clone(),
                        _ => return Err(error("HTTP options must be a table")),
                    }
                };
                let body = match options.get::<Value>("body")? {
                    Value::Nil => None,
                    Value::String(s) => Some(s.as_bytes().to_vec()),
                    _ => return Err(error("HTTP body must be a string")),
                };
                let fields = lua.create_table()?;
                for pair in options.pairs::<Value, Value>() {
                    let (key, value) = pair?;
                    if !matches!(&key, Value::String(s) if &*s.as_bytes() == b"body") {
                        fields.set(key, value)?;
                    }
                }
                let opts: Json = lua.from_value(Value::Table(fields))?;
                validate_http(&caps, &opts, body.as_deref()).map_err(error)?;
                let mut s = s.borrow_mut();
                if s.pending.len() >= 64 { return Err(error("too many undelivered asynchronous results")); }
                if s.pending.iter().filter(|p| p.kind == "http").count()
                    >= limit(&caps, "http", "max_pending", 4) as usize {
                    return Err(error("too many http requests"));
                }
                let id = s.next_id;
                s.next_id += 1;
                let max = opts["max_response_bytes"].as_u64()
                    .unwrap_or(limit(&caps, "http", "default_response_bytes", 8192)) as usize;
                let timeout = opts["timeout_ms"].as_u64()
                    .unwrap_or(limit(&caps, "http", "default_timeout_ms", 10000));
                let mut result = s.http_fixture.clone();
                if result.as_ref().is_some_and(|r| r["body"].as_str().is_some_and(|b| b.len() > max)) {
                    result = Some(json!({"error":{"code":"response_too_large","message":"Response exceeds requested limit"}}));
                }
                let http = result.is_none().then(|| crate::network::Transfer::new(opts.clone(), max, timeout, body));
                // Query strings and headers can contain credentials; do not log them.
                s.log(format!("HTTP #{id}: {} ({})", opts["method"].as_str().unwrap_or("GET"),
                    if http.is_some() { "network" } else { "fixture" }));
                let created = s.elapsed;
                s.pending.push(Pending { id, kind: "http", created, result, http });
                Ok(id)
            })?;
            table.set(if get { "get" } else { "request" }, f)?;
        }
        let s = self.state.clone();
        table.set(
            "cancel",
            self.lua.create_function(move |_, id: u64| {
                let mut s = s.borrow_mut();
                let n = s.pending.len();
                s.pending.retain(|p| !(p.id == id && p.kind == "http"));
                Ok(s.pending.len() != n)
            })?,
        )?;
        self.lua.globals().set("http", table)?;
        Ok(())
    }
    fn register_tts(&self) -> Result<()> {
        let t = self.lua.create_table()?;
        let s = self.state.clone();
        let caps = self.caps.clone();
        t.set(
            "speak",
            self.lua.create_function(move |_, value: Value| {
                let mut s = s.borrow_mut();
                if s.pending.len() >= 64 {
                    return Err(error("too many undelivered asynchronous results"));
                }
                let id = s.next_id;
                s.next_id += 1;
                let text = match value {
                    Value::String(s) => s.to_str().ok().map(|s| s.to_owned()),
                    _ => None,
                };
                if s.staging || s.exiting {
                    return Err(error("tts unavailable during startup or exit"));
                }
                let text = text.ok_or_else(|| error("TTS text must be a string"))?;
                if text.is_empty()
                    || text.contains('\0')
                    || text.len() > limit(&caps, "tts", "text_max_bytes", 512) as usize
                {
                    return Err(error("invalid TTS text length"));
                }
                if s.pending.iter().filter(|p| p.kind == "tts").count() >= 4 {
                    return Err(error("too many pending TTS results"));
                }
                let busy = s.speech.is_some();
                let result = busy
                    .then(|| json!({"status":"failed","error":{"code":"busy","message":"busy"}}));
                if !busy {
                    s.log(format!("TTS #{id}: {text}"));
                    s.speech = Some((id, text));
                }
                let created = s.elapsed;
                s.pending.push(Pending {
                    id,
                    kind: "tts",
                    created,
                    result,
                    http: None,
                });
                Ok(id)
            })?,
        )?;
        let s = self.state.clone();
        t.set(
            "cancel",
            self.lua.create_function(move |_, id: u64| {
                let mut s = s.borrow_mut();
                let n = s.pending.len();
                s.pending.retain(|p| !(p.id == id && p.kind == "tts"));
                if s.speech.as_ref().is_some_and(|(i, _)| *i == id) {
                    s.speech = None;
                }
                Ok(n != s.pending.len())
            })?,
        )?;
        self.lua.globals().set("tts", t)?;
        Ok(())
    }
    fn invoke<A: mlua::IntoLuaMulti>(&self, name: &str, args: A) -> Result<()> {
        let value = self.lua.globals().get::<Value>(name)?;
        if let Value::Function(f) = value {
            self.budget.set(250_000);
            self.wall_limit.set(500);
            self.deadline.set(Instant::now());
            f.call::<()>(args)?;
        } else if !matches!(value, Value::Nil) {
            return Err(error(format!("{name} is not a function")));
        }
        Ok(())
    }
    fn check(&mut self, result: Result<()>) {
        if let Err(e) = result {
            self.fault = Some(e.to_string());
            self.stopped = true;
            let mut s = self.state.borrow_mut();
            s.pending.clear();
            s.tones.clear();
            s.speech = None;
            for led in s.leds.values_mut() {
                *led = Led::Off;
            }
            s.log(e.to_string());
            if s.read_storage {
                let mut store = s.store.borrow_mut();
                if let Some(e) = store.entries.get_mut(&s.id) {
                    e.expires = 0;
                }
                let _ = store.flush();
            }
        }
    }
    pub fn tick(&mut self, dt: u64) {
        if self.stopped {
            return;
        }
        self.state.borrow_mut().elapsed += dt;
        let result = self.invoke("on_tick", dt);
        self.check(result);
        if self.stopped {
            return;
        }
        let results = {
            let mut s = self.state.borrow_mut();
            let now = s.elapsed;
            for p in &mut s.pending {
                if let Some(http) = &mut p.http {
                    http.poll();
                }
                if p.kind == "tts"
                    && p.result.is_none()
                    && now - p.created >= limit(&self.caps, "tts", "timeout_ms", 120000)
                {
                    p.result = Some(
                        json!({"status":"failed","error":{"code":"timeout","message":"TTS timed out"}}),
                    );
                }
            }
            s.pending
                .iter()
                .filter(|p| p.result.is_some() || p.http.as_ref().is_some_and(|h| h.ready()))
                .map(|p| p.id)
                .collect::<Vec<_>>()
        };
        for id in results {
            if self.stopped {
                break;
            }
            let mut p = {
                let mut state = self.state.borrow_mut();
                let Some(index) = state.pending.iter().position(|p| p.id == id) else {
                    continue;
                };
                state.pending.remove(index)
            };
            let callback = if p.kind == "http" {
                "on_http_response"
            } else {
                "on_tts_result"
            };
            if p.kind == "tts"
                && self
                    .state
                    .borrow()
                    .speech
                    .as_ref()
                    .is_some_and(|(i, _)| *i == p.id)
            {
                self.state.borrow_mut().speech = None;
            }
            let value = match p.http.as_mut() {
                Some(http) => http.take_response().into_lua(&self.lua),
                None => self.lua.to_value(&p.result.unwrap()),
            };
            let result = value.and_then(|v| self.invoke(callback, (p.id, v)));
            self.check(result);
        }
    }
    pub fn click(&mut self, id: &str) {
        if !self.stopped && self.caps.items("buttons").iter().any(|b| b["id"] == id) {
            let r = self.invoke("on_button_click", id);
            self.check(r);
        }
    }
    pub fn finish_speech(&mut self, status: &str) {
        if !matches!(status, "completed" | "interrupted" | "failed") {
            return;
        }
        let mut s = self.state.borrow_mut();
        if let Some((id, _)) = s.speech.take()
            && let Some(p) = s.pending.iter_mut().find(|p| p.id == id)
        {
            p.result = Some(json!({"status":status}));
        }
    }
    pub fn stop(&mut self) {
        if self.stopped {
            return;
        }
        self.state.borrow_mut().exiting = true;
        let r = self.invoke("on_exit", ());
        self.check(r);
        self.stopped = true;
        let mut s = self.state.borrow_mut();
        s.pending.clear();
        s.tones.clear();
        s.speech = None;
        for l in s.leds.values_mut() {
            *l = Led::Off;
        }
    }
    pub fn eval<T: mlua::FromLuaMulti>(&self, expression: &str) -> Result<T> {
        self.lua.load(expression).eval()
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.state.borrow_mut().pending.clear();
    }
}
fn validate_http(
    c: &Capabilities,
    o: &Json,
    body: Option<&[u8]>,
) -> std::result::Result<(), &'static str> {
    let url = o["url"].as_str().ok_or("invalid_argument")?;
    if !(url.starts_with("https://") || url.starts_with("http://"))
        || url.len() > limit(c, "http", "max_url_bytes", 511) as usize
        || url.contains(['\0', '\r', '\n'])
    {
        return Err("invalid_argument");
    }
    let method = match o.get("method") {
        None => "GET",
        Some(value) => value.as_str().ok_or("invalid_argument")?,
    };
    if !c.sdk()["http"]["methods"]
        .as_array()
        .is_some_and(|a| a.iter().any(|v| v == method))
    {
        return Err("invalid_argument");
    }
    if let Some(body) = body
        && (method != "POST" || body.len() > limit(c, "http", "max_body_bytes", 8192) as usize)
    {
        return Err("invalid_argument");
    }
    if let Some(headers) = o.get("headers") {
        let h = headers.as_object().ok_or("invalid_argument")?;
        let mut bytes = 0;
        if h.len() > limit(c, "http", "max_headers", 16) as usize {
            return Err("invalid_argument");
        }
        for (k, v) in h {
            let v = v.as_str().ok_or("invalid_argument")?;
            if k.is_empty() || k.contains(['\0', '\r', '\n', ':']) || v.contains(['\0', '\r', '\n'])
            {
                return Err("invalid_argument");
            }
            bytes += k.len() + v.len() + 4;
        }
        if bytes > limit(c, "http", "max_headers_bytes", 1024) as usize {
            return Err("invalid_argument");
        }
    }
    for (key, max) in [
        ("timeout_ms", "max_timeout_ms"),
        ("max_response_bytes", "max_response_bytes"),
    ] {
        if let Some(v) = o.get(key)
            && !v
                .as_u64()
                .is_some_and(|n| n > 0 && n <= limit(c, "http", max, 0))
        {
            return Err("invalid_argument");
        }
    }
    Ok(())
}
