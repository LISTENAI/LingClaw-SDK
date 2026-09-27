mod profiles;
use gpui_kit::assets::IconName as Glyph;
use gpui_kit::component::{
    button::*,
    input::{Editor, EditorState, Input, InputState},
    *,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use lingclaw_sdk::{
    capabilities::Capabilities,
    render,
    runtime::{Led, Runtime, Store},
};
use profiles::Profiles;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

fn default_auto_reload() -> bool {
    true
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Preferences {
    #[serde(default, skip_serializing)]
    capabilities: serde_json::Value,
    #[serde(default, skip_serializing)]
    hotkeys: BTreeMap<String, String>,
    #[serde(default)]
    profiles: Option<Profiles>,
    zoom: u32,
    #[serde(default)]
    last_file: Option<PathBuf>,
    #[serde(default = "default_auto_reload")]
    auto_reload: bool,
    #[serde(default)]
    muted: bool,
}
fn preferences_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "ListenAI", "LingClaw")
        .map(|p| p.config_dir().join("studio.json"))
}
actions!(lingclaw, [Quit, OpenLua, ReloadLua]);

const EXAMPLE: &str = include_str!("../../examples/counter.lua");
struct Studio {
    caps: Capabilities,
    profiles: Profiles,
    profile_name: Entity<InputState>,
    runtime: Option<Runtime>,
    store: Rc<RefCell<Store>>,
    source: String,
    watcher: Option<lingclaw_sdk::watch::SourceWatch>,
    auto_reload: bool,
    last_scan: Instant,
    settings: bool,
    dialog_open: bool,
    audio: Option<lingclaw_sdk::audio::Audio>,
    audio_error: Option<String>,
    muted: bool,
    config: Entity<EditorState>,
    width_input: Entity<InputState>,
    height_input: Entity<InputState>,
    item_id: Entity<InputState>,
    item_label: Entity<InputState>,
    adding_led: bool,
    recording: Option<String>,
    tab: usize,
    show_log: bool,
    zoom: u32,
    paused: bool,
    focus: FocusHandle,
    held: BTreeMap<String, Instant>,
    hotkeys: BTreeMap<String, String>,
    path: Option<PathBuf>,
    message: String,
    image: Option<Arc<RenderImage>>,
    pixels: Option<image::RgbaImage>,
    frame: u64,
}
impl Studio {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let preferences = preferences_path()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|bytes| serde_json::from_slice::<Preferences>(&bytes).ok())
            .filter(|p| matches!(p.zoom, 1 | 2 | 4));
        let profiles = preferences
            .as_ref()
            .and_then(|p| {
                if let Some(profiles) = &p.profiles {
                    profiles.validate().ok().map(|_| profiles.clone())
                } else {
                    Profiles::migrate(p.capabilities.clone(), p.hotkeys.clone()).ok()
                }
            })
            .unwrap_or_default();
        let caps = profiles.capabilities();
        let hotkeys = profiles.hotkeys();
        let zoom = preferences.as_ref().map(|p| p.zoom).unwrap_or(2);
        let text = |s: String, language: &str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| {
                let mut t = EditorState::new(window, cx)
                    .language(language.to_owned())
                    .line_number(true)
                    .soft_wrap(false);
                t.set_value(s, window, cx);
                t
            })
        };
        let config = text(
            serde_json::to_string_pretty(&caps.0).unwrap(),
            "json",
            window,
            cx,
        );
        let mut input = |value: String, hint: &str, cx: &mut Context<Self>| {
            cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(hint.to_owned())
                    .default_value(value)
            })
        };
        let profile_name = input(profiles.name().to_owned(), "设备名称", cx);
        let width_input = input(caps.width().to_string(), "宽度", cx);
        let height_input = input(caps.height().to_string(), "高度", cx);
        let item_id = input(String::new(), "代码标识，如 left", cx);
        let item_label = input(String::new(), "显示名称，如 左键", cx);
        let path = directories::ProjectDirs::from("com", "ListenAI", "LingClaw")
            .map(|p| p.data_local_dir().join("storage.json"));
        let store = Rc::new(RefCell::new(path.map(Store::open).unwrap_or_default()));
        let (audio, audio_error) = match lingclaw_sdk::audio::Audio::new() {
            Ok(audio) => (Some(audio), None),
            Err(error) => (None, Some(error.to_string())),
        };
        let mut s = Self {
            caps,
            profiles,
            profile_name,
            runtime: None,
            store,
            source: EXAMPLE.into(),
            watcher: None,
            auto_reload: preferences.as_ref().map(|p| p.auto_reload).unwrap_or(true),
            last_scan: Instant::now(),
            settings: false,
            dialog_open: false,
            audio,
            audio_error,
            muted: preferences.as_ref().is_some_and(|p| p.muted),
            config,
            width_input,
            height_input,
            item_id,
            item_label,
            adding_led: false,
            recording: None,
            tab: 0,
            show_log: false,
            zoom,
            paused: false,
            focus: cx.focus_handle(),
            held: BTreeMap::new(),
            hotkeys,
            path: None,
            message: "就绪".into(),
            image: None,
            pixels: None,
            frame: u64::MAX,
        };
        if let Some(path) = std::env::args()
            .nth(1)
            .map(PathBuf::from)
            .or_else(|| preferences.as_ref().and_then(|p| p.last_file.clone()))
        {
            let path = path.canonicalize().unwrap_or(path);
            match lingclaw_sdk::watch::read_source(&path) {
                Ok(source) => {
                    s.watcher = Some(lingclaw_sdk::watch::SourceWatch::new(
                        path.clone(),
                        source.clone(),
                    ));
                    s.path = Some(path);
                    s.source = source;
                }
                Err(e) => s.message = format!("读取文件失败：{e}"),
            }
        }
        let opening_error = s
            .message
            .starts_with("读取文件失败")
            .then(|| s.message.clone());
        s.run(cx);
        if let Some(error) = opening_error {
            s.message = error;
        }
        cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(20))
                    .await;
                if view
                    .update(cx, |s, cx| {
                        let before = s.visual_state();
                        if s.auto_reload && s.last_scan.elapsed() >= Duration::from_millis(250) {
                            s.last_scan = Instant::now();
                            if let Some(watcher) = &mut s.watcher {
                                match watcher.poll(Instant::now()) {
                                    Ok(Some(source)) => {
                                        s.source = source;
                                        s.run(cx);
                                    }
                                    Err(e) => s.message = format!("读取文件失败：{e}"),
                                    Ok(None) => {
                                        if s.message.starts_with("读取文件失败") {
                                            s.message = "运行中".into();
                                        }
                                    }
                                }
                            }
                        }
                        if !s.paused
                            && let Some(r) = &mut s.runtime
                        {
                            r.tick(20);
                        }
                        for (id, since) in &s.held {
                            let hold = s
                                .caps
                                .items("buttons")
                                .iter()
                                .find(|b| b["id"] == *id)
                                .and_then(|b| b["reserved_hold_ms"].as_u64());
                            if hold.is_some_and(|ms| since.elapsed().as_millis() >= ms as u128)
                                && let Some(r) = &mut s.runtime
                            {
                                r.stop();
                            }
                        }
                        s.refresh();
                        if s.visual_state() != before {
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        s
    }
    fn visual_state(&self) -> String {
        let Some(runtime) = &self.runtime else {
            return self.message.clone();
        };
        let state = runtime.state.borrow();
        let leds = state
            .leds
            .iter()
            .map(|(id, led)| {
                let on = match led {
                    Led::On => true,
                    Led::Off => false,
                    Led::Blink(on, off, start) => (state.elapsed - start) % (on + off) < *on,
                };
                (id, on)
            })
            .collect::<Vec<_>>();
        format!(
            "{}:{}:{}:{:?}:{:?}:{:?}",
            state.frames,
            runtime.stopped,
            self.message,
            state.speech,
            leds,
            state.log.back()
        )
    }
    fn persist_preferences(&mut self) {
        let result = (|| -> anyhow::Result<()> {
            self.profiles.set_hotkeys(self.hotkeys.clone())?;
            let path = preferences_path().ok_or_else(|| anyhow::anyhow!("无法找到配置目录"))?;
            std::fs::create_dir_all(path.parent().unwrap())?;
            std::fs::write(
                path,
                serde_json::to_vec_pretty(&Preferences {
                    capabilities: serde_json::Value::Null,
                    hotkeys: BTreeMap::new(),
                    profiles: Some(self.profiles.clone()),
                    zoom: self.zoom,
                    last_file: self.path.clone(),
                    auto_reload: self.auto_reload,
                    muted: self.muted,
                })?,
            )?;
            Ok(())
        })();
        if let Err(e) = result {
            self.message = format!("保存配置失败：{e}");
        }
    }
    fn run(&mut self, cx: &mut Context<Self>) {
        let source = self.source.clone();
        let id = self
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("untitled");
        let candidate = Runtime::new(&source, id, self.caps.clone(), self.store.clone());
        match candidate {
            Ok(r) => {
                if let Some(audio) = &mut self.audio {
                    audio.stop();
                }
                // Retired instances are dropped, so on_exit cannot overwrite the new instance's state.
                self.runtime = Some(r);
                self.paused = false;
                self.held.clear();
                self.frame = u64::MAX;
                self.message = "运行中".into();
                self.refresh();
            }
            Err(e) => self.message = format!("启动失败：{e}"),
        }
        cx.notify();
    }
    fn refresh(&mut self) {
        if let Some(runtime) = &self.runtime {
            let mut state = runtime.state.borrow_mut();
            if runtime.stopped || state.speech.is_some() || self.muted {
                state.tones.clear();
                if let Some(audio) = &mut self.audio {
                    audio.stop();
                }
            } else {
                for tone in state.tones.drain(..) {
                    if let Some(audio) = &self.audio {
                        audio.play(tone);
                    }
                }
            }
        }
        let Some(r) = &self.runtime else {
            return;
        };
        if let Some(error) = &r.fault {
            self.message = format!("Lua 异常：{error}");
        }
        if !self.caps.has_screen() {
            self.image = None;
            self.pixels = None;
            return;
        }
        let s = r.state.borrow();
        if s.frames == self.frame {
            return;
        }
        match render::render(self.caps.width(), self.caps.height(), &s.scene) {
            Ok(pixels) => {
                let bgra = match render::preview_bgra(&pixels, self.zoom) {
                    Ok(bgra) => bgra,
                    Err(error) => {
                        self.message = format!("预览失败：{error}");
                        return;
                    }
                };
                self.image = Some(Arc::new(RenderImage::new(vec![image::Frame::new(bgra)])));
                self.pixels = Some(pixels);
                self.frame = s.frames;
            }
            Err(e) => self.message = format!("操作失败：{e}"),
        }
    }
    fn sync_profile_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.config.update(cx, |t, cx| {
            t.set_value(
                serde_json::to_string_pretty(&self.caps.0).unwrap(),
                window,
                cx,
            )
        });
        self.width_input.update(cx, |t, cx| {
            t.set_value(self.caps.width().to_string(), window, cx)
        });
        self.height_input.update(cx, |t, cx| {
            t.set_value(self.caps.height().to_string(), window, cx)
        });
        self.profile_name.update(cx, |t, cx| {
            t.set_value(self.profiles.name().to_owned(), window, cx)
        });
        self.recording = None;
        self.held.clear();
    }
    fn switch_profile(&mut self, profiles: Profiles, window: &mut Window, cx: &mut Context<Self>) {
        let previous_caps = self.caps.clone();
        let previous_keys = self.hotkeys.clone();
        self.caps = profiles.capabilities();
        self.hotkeys = profiles.hotkeys();
        self.run(cx);
        if self.message.starts_with("启动失败") {
            self.caps = previous_caps;
            self.hotkeys = previous_keys;
        } else {
            self.profiles = profiles;
            self.persist_preferences();
        }
        self.sync_profile_inputs(window, cx);
        cx.notify();
    }
    fn clone_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut profiles = self.profiles.clone();
        profiles.clone_active();
        self.switch_profile(profiles, window, cx);
        self.tab = 0;
    }
    fn load_profile(&mut self, caps: Capabilities, window: &mut Window, cx: &mut Context<Self>) {
        let mut profiles = self.profiles.clone();
        match profiles.update(caps) {
            Ok(()) => self.switch_profile(profiles, window, cx),
            Err(e) => {
                self.message = format!("配置失败：{e}");
                cx.notify();
            }
        }
    }
    fn apply(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match Capabilities::parse(&self.config.read(cx).value()) {
            Ok(caps) => self.load_profile(caps, window, cx),
            Err(e) => {
                self.message = format!("配置失败：{e}");
                cx.notify();
            }
        }
    }
    fn resize_screen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dimensions = self
            .width_input
            .read(cx)
            .value()
            .parse::<u32>()
            .and_then(|w| {
                self.height_input
                    .read(cx)
                    .value()
                    .parse::<u32>()
                    .map(|h| (w, h))
            });
        match dimensions {
            Ok((w, h)) if (1..=2048).contains(&w) && (1..=2048).contains(&h) => {
                let mut caps = self.caps.clone();
                caps.0["hardware"]["display"]["width_px"] = w.into();
                caps.0["hardware"]["display"]["height_px"] = h.into();
                self.load_profile(caps, window, cx);
            }
            _ => {
                self.message = "配置失败：屏幕宽高应为 1–2048 的整数".into();
                cx.notify();
            }
        }
    }
    fn add_item(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.item_id.read(cx).value().trim().to_owned();
        let label = self.item_label.read(cx).value().trim().to_owned();
        let kind = if self.adding_led { "leds" } else { "buttons" };
        if id.is_empty()
            || id.len() > 127
            || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || label.is_empty()
            || self.caps.items(kind).iter().any(|v| v["id"] == id)
        {
            self.message = "配置失败：请填写名称与唯一标识（字母、数字、下划线）".into();
            cx.notify();
            return;
        }
        let mut items = self.caps.items(kind).to_vec();
        items.push(if self.adding_led {
            serde_json::json!({"id":id,"label":label,"color":false})
        } else {
            serde_json::json!({"id":id,"label":label,"events":["click"]})
        });
        let mut caps = self.caps.clone();
        let group = if self.adding_led { "lighting" } else { "input" };
        caps.0["hardware"][group][kind] = items.into();
        self.load_profile(caps, window, cx);
        if !self.message.contains("失败") {
            self.item_id.update(cx, |t, cx| t.set_value("", window, cx));
            self.item_label
                .update(cx, |t, cx| t.set_value("", window, cx));
        }
    }
    fn remove_item(&mut self, kind: &str, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let mut caps = self.caps.clone();
        let items = caps
            .items(kind)
            .iter()
            .filter(|v| v["id"] != id)
            .cloned()
            .collect::<Vec<_>>();
        let group = if kind == "leds" { "lighting" } else { "input" };
        caps.0["hardware"][group][kind] = items.into();
        self.load_profile(caps, window, cx);
    }
    fn item_form(&self, cx: &mut Context<Self>) -> Div {
        div()
            .v_flex()
            .gap_2()
            .child(Input::new(&self.item_id).small())
            .child(Input::new(&self.item_label).small())
            .child(
                Button::new("add-hardware")
                    .small()
                    .label(if self.adding_led {
                        "添加指示灯"
                    } else {
                        "添加按键"
                    })
                    .on_click(cx.listener(|s, _, w, cx| s.add_item(w, cx))),
            )
    }
    fn settings_panel(&self, speech: Option<(u64, String)>, cx: &mut Context<Self>) -> Div {
        let mut tabs = div().flex().gap_1();
        for (tab, label) in ["硬件", "按键映射", "播报", "高级"].into_iter().enumerate() {
            tabs = tabs.child(
                Button::new(("panel-tab", tab))
                    .ghost()
                    .small()
                    .label(label)
                    .selected(self.tab == tab)
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.tab = tab;
                        s.recording = None;
                        if tab == 1 {
                            s.adding_led = false;
                        }
                        cx.notify();
                    })),
            );
        }
        let mut content = div()
            .id("settings-scroll")
            .v_flex()
            .gap_5()
            .p_4()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        match self.tab {
            0 => {
                if self.profiles.builtin() {
                    content = content.child(format!(
                        "屏幕  {} × {}",
                        self.caps.width(),
                        self.caps.height()
                    ));
                } else {
                    content = content.child(
                        div()
                            .flex()
                            .gap_2()
                            .child(div().flex_1().child(Input::new(&self.profile_name).small()))
                            .child(
                                Button::new("rename-device")
                                    .small()
                                    .label("重命名")
                                    .on_click(cx.listener(|s, _, _, cx| {
                                        let name = s.profile_name.read(cx).value().to_string();
                                        if let Err(e) = s.profiles.rename(&name) {
                                            s.message = format!("配置失败：{e}");
                                        } else {
                                            s.persist_preferences();
                                        }
                                        cx.notify();
                                    })),
                            ),
                    );
                    content = content.child(
                        div().v_flex().gap_2().child("屏幕尺寸").child(
                            div()
                                .flex()
                                .gap_2()
                                .items_center()
                                .child(div().flex_1().child(Input::new(&self.width_input).small()))
                                .child("×")
                                .child(div().flex_1().child(Input::new(&self.height_input).small()))
                                .child(
                                    Button::new("resize-screen").small().label("应用").on_click(
                                        cx.listener(|s, _, w, cx| s.resize_screen(w, cx)),
                                    ),
                                ),
                        ),
                    );
                }
                for (kind, title) in [("buttons", "按键"), ("leds", "指示灯")] {
                    let mut section = div().v_flex().gap_2().child(title);
                    for item in self.caps.items(kind) {
                        let id = item["id"].as_str().unwrap().to_owned();
                        let label = item["label"].as_str().unwrap_or(&id).to_owned();
                        section =
                            section.child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .py_2()
                                    .border_b_1()
                                    .border_color(rgb(0x2a323b))
                                    .child(div().v_flex().gap_1().child(label).child(
                                        div().text_xs().text_color(rgb(0x89939d)).child(id.clone()),
                                    ))
                                    .when(!self.profiles.builtin(), |row| {
                                        row.child(
                                            Button::new(SharedString::from(format!(
                                                "remove-{kind}-{id}"
                                            )))
                                            .ghost()
                                            .small()
                                            .label("移除")
                                            .on_click(cx.listener(move |s, _, w, cx| {
                                                s.remove_item(kind, &id, w, cx)
                                            })),
                                        )
                                    }),
                            );
                    }
                    if self.caps.items(kind).is_empty() {
                        section = section
                            .child(div().text_xs().text_color(rgb(0x89939d)).child("尚未配置"));
                    }
                    content = content.child(section);
                }
                if !self.profiles.builtin() {
                    content = content.child(
                        div()
                            .v_flex()
                            .gap_3()
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(
                                        Button::new("new-button")
                                            .small()
                                            .ghost()
                                            .label("新增按键")
                                            .selected(!self.adding_led)
                                            .on_click(cx.listener(|s, _, _, cx| {
                                                s.adding_led = false;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("new-led")
                                            .small()
                                            .ghost()
                                            .label("新增指示灯")
                                            .selected(self.adding_led)
                                            .on_click(cx.listener(|s, _, _, cx| {
                                                s.adding_led = true;
                                                cx.notify();
                                            })),
                                    ),
                            )
                            .child(self.item_form(cx)),
                    );
                    content = content.child(
                        Button::new("delete-device")
                            .ghost()
                            .small()
                            .label("删除此自定义设备")
                            .on_click(cx.listener(|s, _, w, cx| {
                                let mut profiles = s.profiles.clone();
                                if profiles.remove_active().is_ok() {
                                    s.switch_profile(profiles, w, cx);
                                }
                            })),
                    );
                }
            }
            1 => {
                content = content.child(
                    div().v_flex().gap_1().child("键盘映射").child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x89939d))
                            .child("点击右侧录入快捷键，Esc 取消。清除仅解除映射。"),
                    ),
                );
                for item in self.caps.items("buttons") {
                    let id = item["id"].as_str().unwrap().to_owned();
                    let record_id = id.clone();
                    let clear_id = id.clone();
                    let recording = self.recording.as_ref() == Some(&id);
                    let label = item["label"].as_str().unwrap_or(&id).to_owned();
                    let key = if recording {
                        "请按键…".to_owned()
                    } else {
                        self.hotkeys
                            .get(&id)
                            .map(|key| {
                                if key == "space" {
                                    "Space".into()
                                } else {
                                    key.clone()
                                }
                            })
                            .unwrap_or("未绑定".into())
                    };
                    content = content.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .py_2()
                            .border_b_1()
                            .border_color(rgb(0x2a323b))
                            .child(
                                div()
                                    .v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_1()
                                    .child(label)
                                    .child(
                                        div().text_xs().text_color(rgb(0x89939d)).child(id.clone()),
                                    ),
                            )
                            .child(
                                Button::new(SharedString::from(format!("record-{id}")))
                                    .small()
                                    .label(key)
                                    .when(recording, |b| b.primary())
                                    .on_click(cx.listener(move |s, _, w, cx| {
                                        s.held.clear();
                                        s.recording = Some(record_id.clone());
                                        s.message = "请按下快捷键，Esc 取消".into();
                                        s.focus.focus(w, cx);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new(SharedString::from(format!("clear-{id}")))
                                    .small()
                                    .ghost()
                                    .label("清除")
                                    .on_click(cx.listener(move |s, _, _, cx| {
                                        s.hotkeys.remove(&clear_id);
                                        s.recording = None;
                                        s.held.clear();
                                        s.persist_preferences();
                                        cx.notify();
                                    })),
                            ),
                    );
                }
            }
            2 => {
                content = content.child(
                    div().v_flex().gap_1().child("语音播报").child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x89939d))
                            .child("选择播报结果以测试应用的回调处理。"),
                    ),
                );
                if let Some((id, text)) = speech {
                    content = content.child(
                        div()
                            .v_flex()
                            .gap_3()
                            .p_4()
                            .rounded_lg()
                            .bg(rgb(0x202a2c))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0xa7f3d0))
                                    .child(format!("等待结果 · #{id}")),
                            )
                            .child(text)
                            .child(
                                div().flex().gap_2().flex_wrap().children(
                                    [
                                        ("completed", "完成"),
                                        ("interrupted", "中断"),
                                        ("failed", "失败"),
                                    ]
                                    .map(|(state, label)| {
                                        Button::new(state).small().label(label).on_click(
                                            cx.listener(move |s, _, _, cx| {
                                                if let Some(r) = &mut s.runtime {
                                                    r.finish_speech(state);
                                                }
                                                cx.notify();
                                            }),
                                        )
                                    }),
                                ),
                            ),
                    );
                } else {
                    content = content.child(
                        div()
                            .p_4()
                            .rounded_lg()
                            .bg(rgb(0x111519))
                            .text_color(rgb(0x89939d))
                            .child("当前没有待处理的播报"),
                    );
                }
            }
            _ if self.profiles.builtin() => {
                content = content.child("复制配置后可编辑更多参数。").child(
                    Button::new("export-mini")
                        .small()
                        .label("导出能力描述")
                        .on_click(cx.listener(|s, _, w, cx| s.export(false, w, cx))),
                );
            }
            _ => {
                content = content
                    .child(
                        div().v_flex().gap_1().child("原始能力描述").child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x89939d))
                                .child("编辑 API 版本、资源额度及硬件参数。"),
                        ),
                    )
                    .child(
                        div().h(px(420.)).child(
                            Editor::new(&self.config)
                                .appearance(false)
                                .bordered(false)
                                .h(relative(1.)),
                        ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("apply-advanced")
                                    .small()
                                    .label("应用配置")
                                    .on_click(cx.listener(|s, _, w, cx| s.apply(w, cx))),
                            )
                            .child(
                                Button::new("import-capabilities")
                                    .small()
                                    .ghost()
                                    .label("导入")
                                    .on_click(cx.listener(|s, _, w, cx| s.open(true, w, cx))),
                            )
                            .child(
                                Button::new("export-capabilities")
                                    .small()
                                    .ghost()
                                    .label("导出")
                                    .on_click(cx.listener(|s, _, w, cx| s.export(false, w, cx))),
                            ),
                    );
            }
        }
        let mut devices = div().v_flex().gap_1();
        let options = std::iter::once(("mini".to_owned(), "Mini".to_owned())).chain(
            self.profiles
                .custom()
                .iter()
                .map(|p| (p.id.clone(), p.name.clone())),
        );
        for (id, name) in options {
            let selected = self.profiles.active_id() == id;
            devices = devices.child(
                Button::new(SharedString::from(format!("device-{id}")))
                    .ghost()
                    .small()
                    .label(name)
                    .selected(selected)
                    .on_click(cx.listener(move |s, _, w, cx| {
                        if s.profiles.active_id() == id {
                            return;
                        }
                        let mut profiles = s.profiles.clone();
                        if profiles.select(&id).is_ok() {
                            s.switch_profile(profiles, w, cx);
                        }
                    })),
            );
        }
        div()
            .v_flex()
            .w(px(380.))
            .flex_none()
            .min_h_0()
            .bg(rgb(0x191e24))
            .border_l_1()
            .border_color(rgb(0x2a323b))
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .p_3()
                    .border_b_1()
                    .border_color(rgb(0x2a323b))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .child("设备配置")
                            .child(
                                Button::new("clone-device")
                                    .small()
                                    .label("复制配置")
                                    .on_click(cx.listener(|s, _, w, cx| s.clone_profile(w, cx))),
                            ),
                    )
                    .child(
                        div()
                            .id("device-list")
                            .max_h(px(130.))
                            .overflow_y_scroll()
                            .child(devices),
                    ),
            )
            .child(
                div()
                    .h(px(56.))
                    .flex_none()
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(tabs)
                    .child(
                        Button::new("close-panel")
                            .ghost()
                            .small()
                            .label("收起")
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.settings = false;
                                s.recording = None;
                                cx.notify();
                            })),
                    ),
            )
            .child(content)
    }
    fn reload(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = &self.path {
            match lingclaw_sdk::watch::read_source(path) {
                Ok(source) => {
                    self.watcher = Some(lingclaw_sdk::watch::SourceWatch::new(
                        path.clone(),
                        source.clone(),
                    ));
                    self.source = source;
                }
                Err(e) => {
                    self.message = format!("读取文件失败：{e}");
                    return;
                }
            }
        }
        self.run(cx);
    }
    fn open(&mut self, capabilities: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialog_open {
            return;
        }
        self.dialog_open = true;
        let selection = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(
                if capabilities {
                    "导入配置"
                } else {
                    "打开 Lua"
                }
                .into(),
            ),
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = selection.await;
            let _ = cx.update(|window, app| {
                view.update(app, |s, cx| {
                    s.dialog_open = false;
                    let result = (|| -> anyhow::Result<()> {
                        let Some(paths) = result?? else {
                            return Ok(());
                        };
                        let Some(path) = paths.into_iter().next() else {
                            return Ok(());
                        };
                        if capabilities {
                            let c = Capabilities::parse(&std::fs::read_to_string(path)?)?;
                            s.load_profile(c, window, cx);
                        } else {
                            let source = lingclaw_sdk::watch::read_source(&path)?;
                            s.watcher = Some(lingclaw_sdk::watch::SourceWatch::new(
                                path.clone(),
                                source.clone(),
                            ));
                            s.path = Some(path);
                            s.source = source;
                            s.run(cx);
                        }
                        Ok(())
                    })();
                    if let Err(e) = result {
                        s.message = format!("打开失败：{e}");
                    }
                    cx.notify();
                })
            });
        })
        .detach();
    }
    fn export(&mut self, screenshot: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialog_open {
            return;
        }
        let bytes = if screenshot {
            let Some(pixels) = &self.pixels else {
                return;
            };
            let mut output = std::io::Cursor::new(Vec::new());
            if let Err(e) = pixels.write_to(&mut output, image::ImageFormat::Png) {
                self.message = format!("截图失败：{e}");
                return;
            }
            output.into_inner()
        } else {
            self.config.read(cx).value().as_bytes().to_vec()
        };
        let directory = self
            .path
            .as_ref()
            .and_then(|p| p.parent())
            .map(PathBuf::from)
            .or_else(|| directories::UserDirs::new().map(|p| p.home_dir().to_owned()))
            .unwrap_or_default();
        self.dialog_open = true;
        let selection = cx.prompt_for_new_path(
            &directory,
            Some(if screenshot {
                "screenshot.png"
            } else {
                "capabilities.json"
            }),
        );
        cx.spawn_in(window, async move |view, cx| {
            let result = selection.await;
            let _ = view.update(cx, |s, cx| {
                s.dialog_open = false;
                let result = (|| -> anyhow::Result<()> {
                    if let Some(path) = result?? {
                        std::fs::write(path, bytes)?;
                        s.message = "已保存".into();
                    }
                    Ok(())
                })();
                if let Err(e) = result {
                    s.message = format!("保存失败：{e}");
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl Render for Studio {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focus.is_focused(window) {
            self.held.clear();
        }
        let title = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|p| p.to_str())
            .unwrap_or("counter.lua")
            .to_owned();
        let active = self.runtime.as_ref().is_some_and(|r| !r.stopped);
        let status = if !active {
            "已停止"
        } else if self.paused {
            "已暂停"
        } else {
            "运行中"
        };
        let trouble = self.message.contains("失败")
            || self.message.starts_with("Lua")
            || self.message.starts_with("HTTP 响应");
        let mut zooms = div()
            .flex()
            .items_center()
            .gap_1()
            .p_1()
            .rounded_lg()
            .bg(rgb(0x202429));
        for z in [1u32, 2, 4] {
            zooms = zooms.child(
                Button::new(("zoom", z))
                    .ghost()
                    .small()
                    .selected(self.zoom == z)
                    .label(format!("{z}×"))
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.zoom = z;
                        s.persist_preferences();
                        s.frame = u64::MAX;
                        s.refresh();
                        cx.notify();
                    })),
            );
        }
        let mut display = div()
            .id("preview")
            .flex_1()
            .min_h_0()
            .overflow_scroll()
            .flex()
            .flex_col()
            .p_6()
            .track_focus(&self.focus)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|s, _, w, cx| s.focus.focus(w, cx)),
            )
            .on_key_down(cx.listener(|s, e: &KeyDownEvent, w, cx| {
                if !s.focus.is_focused(w) {
                    return;
                }
                for (id, key) in &s.hotkeys {
                    if *key == e.keystroke.unparse() && !e.is_held {
                        s.held.insert(id.clone(), Instant::now());
                        cx.notify();
                        cx.stop_propagation();
                    }
                }
            }))
            .on_key_up(cx.listener(|s, e: &KeyUpEvent, w, cx| {
                if !s.focus.is_focused(w) {
                    return;
                }
                let ids = s
                    .hotkeys
                    .iter()
                    .filter(|(_, key)| {
                        Keystroke::parse(key).is_ok_and(|k| k.key == e.keystroke.key)
                    })
                    .map(|(id, _)| id.clone())
                    .collect::<Vec<_>>();
                for id in ids {
                    if let Some(start) = s.held.remove(&id) {
                        let hold = s
                            .caps
                            .items("buttons")
                            .iter()
                            .find(|b| b["id"] == id)
                            .and_then(|b| b["reserved_hold_ms"].as_u64())
                            .unwrap_or(u64::MAX);
                        if start.elapsed().as_millis() < hold as u128
                            && let Some(r) = &mut s.runtime
                        {
                            r.click(&id);
                        }
                    }
                }
                s.refresh();
                cx.notify();
            }));
        if let Some(image) = &self.image {
            display = display.child(
                div()
                    .flex_none()
                    .m_auto()
                    .p(px(10.))
                    .rounded(px(22.))
                    .bg(rgb(0x080a0c))
                    .border_1()
                    .border_color(rgb(0x343b42))
                    .shadow_lg()
                    .child(
                        img(image.clone())
                            .w(px((self.caps.width() * self.zoom) as f32))
                            .h(px((self.caps.height() * self.zoom) as f32)),
                    ),
            );
        } else {
            display = display.child(
                div()
                    .m_auto()
                    .text_color(rgb(0x89939d))
                    .child("当前配置未启用屏幕"),
            );
        }
        let mut buttons = div()
            .flex()
            .justify_center()
            .items_center()
            .gap_3()
            .flex_wrap();
        for button in self.caps.items("buttons").to_vec() {
            let id = button["id"].as_str().unwrap().to_owned();
            let label = button["label"].as_str().unwrap_or(&id).to_owned();
            let key = self
                .hotkeys
                .get(&id)
                .map(|s| if s == "space" { "Space" } else { s })
                .unwrap_or("未绑定");
            buttons = buttons.child(
                Button::new(SharedString::from(format!("device-{id}")))
                    .label(format!("{label}    {key}"))
                    .selected(self.held.contains_key(&id))
                    .when(self.held.contains_key(&id), |button| button.primary())
                    .on_click(cx.listener(move |s, _, w, cx| {
                        if let Some(r) = &mut s.runtime {
                            r.click(&id);
                        }
                        s.focus.focus(w, cx);
                        s.refresh();
                        cx.notify();
                    })),
            );
        }
        let mut leds = div().flex().items_center().gap_4();
        let mut log = String::new();
        let mut speech = None;
        if let Some(r) = &self.runtime {
            let state = r.state.borrow();
            for (id, led) in &state.leds {
                let on = match led {
                    Led::On => true,
                    Led::Off => false,
                    Led::Blink(on, off, start) => (state.elapsed - start) % (on + off) < *on,
                };
                leds = leds.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_xs()
                        .text_color(rgb(0x89939d))
                        .child(div().size(px(6.)).rounded_full().bg(rgb(if on {
                            0xa7f3d0
                        } else {
                            0x47505a
                        })))
                        .child(id.clone()),
                );
            }
            log = state
                .log
                .iter()
                .rev()
                .take(20)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n");
            speech = state.speech.clone();
        }
        let stage = div()
            .v_flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .bg(rgb(0x111519))
            .child(
                div()
                    .h(px(56.))
                    .flex_none()
                    .px_6()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(div().text_sm().text_color(rgb(0xc5cdd5)).child("设备预览"))
                            .child(div().text_xs().text_color(rgb(0x76818d)).child(format!(
                                "{} × {}  ·  API {}",
                                self.caps.width(),
                                self.caps.height(),
                                self.caps.api()
                            ))),
                    )
                    .child(
                        div().flex().items_center().gap_3().child(zooms).child(
                            Button::new("snapshot")
                                .ghost()
                                .small()
                                .icon(Glyph::Camera)
                                .tooltip("导出原始尺寸截图")
                                .on_click(cx.listener(|s, _, w, cx| s.export(true, w, cx))),
                        ),
                    ),
            )
            .child(display)
            .child(
                div().v_flex().gap_3().pb_6().pt_2().child(buttons).child(
                    div()
                        .flex()
                        .justify_center()
                        .text_xs()
                        .text_color(rgb(0x687582))
                        .child("点击预览后使用键盘操作"),
                ),
            );
        let mut workspace = div().flex().flex_1().min_h_0().child(stage);
        let speaking = speech.is_some();
        if self.settings {
            workspace = workspace.child(self.settings_panel(speech, cx));
        }
        let mut root = div()
            .capture_key_down(cx.listener(|s, e: &KeyDownEvent, _, cx| {
                let Some(id) = s.recording.clone() else {
                    return;
                };
                cx.stop_propagation();
                if e.is_held {
                    return;
                }
                if e.keystroke.key == "escape" {
                    s.recording = None;
                    s.message = "已取消快捷键录入".into();
                    cx.notify();
                    return;
                }
                if ["shift", "control", "alt", "platform", "function"]
                    .contains(&e.keystroke.key.as_str())
                {
                    return;
                }
                let key = e.keystroke.unparse();
                if (e.keystroke.modifiers.platform || e.keystroke.modifiers.control)
                    && ["q", "o", "r"].contains(&e.keystroke.key.as_str())
                {
                    s.message = "快捷键设置失败：此组合键用于模拟器操作，请换一个按键".into();
                } else if s
                    .hotkeys
                    .iter()
                    .any(|(other, binding)| *other != id && *binding == key)
                {
                    s.message = "快捷键设置失败：此键已被占用，请先清除原绑定".into();
                } else {
                    s.hotkeys.insert(id, key);
                    s.recording = None;
                    s.persist_preferences();
                    s.message = "快捷键已保存".into();
                }
                cx.notify();
            }))
            .on_action(cx.listener(|s, _: &OpenLua, w, cx| s.open(false, w, cx)))
            .on_action(cx.listener(|s, _: &ReloadLua, _, cx| s.reload(cx)))
            .size_full()
            .v_flex()
            .bg(rgb(0x191e24))
            .text_color(rgb(0xe1e7ed))
            .child(
                div()
                    .h(px(78.))
                    .flex_none()
                    .px_6()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(rgb(0x2a323b))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_4()
                            .child(
                                div()
                                    .size(px(36.))
                                    .rounded_lg()
                                    .bg(rgb(0xb4ead6))
                                    .text_color(rgb(0x17382d))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .font_weight(FontWeight::BOLD)
                                    .text_lg()
                                    .child("L"),
                            )
                            .child(
                                div()
                                    .v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(title),
                                    )
                                    .child(div().text_xs().text_color(rgb(0x85929e)).child(
                                        if self.path.is_some() {
                                            "LingClaw Simulator · 本地文件"
                                        } else {
                                            "LingClaw Simulator · 点击计数示例"
                                        },
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::new("open")
                                    .ghost()
                                    .icon(Glyph::FolderOpen)
                                    .label("打开 Lua")
                                    .on_click(cx.listener(|s, _, w, cx| s.open(false, w, cx))),
                            )
                            .child(
                                Button::new("reload")
                                    .label("重载")
                                    .icon(Glyph::RotateCw)
                                    .on_click(cx.listener(|s, _, _, cx| s.reload(cx))),
                            )
                            .child(
                                Button::new("settings")
                                    .ghost()
                                    .icon(Glyph::Settings)
                                    .selected(self.settings)
                                    .tooltip("设备配置与按键映射")
                                    .on_click(cx.listener(|s, _, _, cx| {
                                        s.settings = !s.settings;
                                        s.recording = None;
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(workspace);
        if trouble {
            root = root.child(
                div()
                    .id("fault")
                    .max_h(px(100.))
                    .overflow_y_scroll()
                    .px_6()
                    .py_3()
                    .bg(rgb(0x39252a))
                    .text_color(rgb(0xf1b1af))
                    .text_sm()
                    .child(self.message.clone()),
            );
        }
        if self.show_log {
            root = root.child(
                div()
                    .h(px(150.))
                    .flex_none()
                    .border_t_1()
                    .border_color(rgb(0x2a323b))
                    .v_flex()
                    .child(
                        div()
                            .px_6()
                            .py_2()
                            .text_xs()
                            .text_color(rgb(0x89939d))
                            .child("事件记录"),
                    )
                    .child(
                        div()
                            .id("logs")
                            .flex_1()
                            .overflow_y_scroll()
                            .px_6()
                            .text_xs()
                            .font_family("monospace")
                            .child(if log.is_empty() {
                                "暂无事件".into()
                            } else {
                                log
                            }),
                    ),
            );
        }
        root.child(
            div()
                .h(px(52.))
                .flex_none()
                .px_5()
                .flex()
                .items_center()
                .justify_between()
                .border_t_1()
                .border_color(rgb(0x2a323b))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(div().size(px(6.)).rounded_full().bg(rgb(
                            if active && !self.paused {
                                0x8adebc
                            } else {
                                0x7f8993
                            },
                        )))
                        .child(div().text_xs().text_color(rgb(0xa8b3bf)).child(status))
                        .child(
                            Button::new("pause")
                                .ghost()
                                .small()
                                .icon(if self.paused {
                                    Glyph::Play
                                } else {
                                    Glyph::Pause
                                })
                                .tooltip(if self.paused {
                                    "继续运行"
                                } else {
                                    "暂停"
                                })
                                .on_click(cx.listener(|s, _, _, cx| {
                                    s.paused = !s.paused;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("step")
                                .ghost()
                                .small()
                                .icon(Glyph::StepForward)
                                .tooltip("单步 20 ms")
                                .on_click(cx.listener(|s, _, _, cx| {
                                    s.paused = true;
                                    if let Some(r) = &mut s.runtime {
                                        r.tick(20);
                                    }
                                    s.refresh();
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("stop")
                                .ghost()
                                .small()
                                .icon(Glyph::Square)
                                .tooltip("退出应用")
                                .on_click(cx.listener(|s, _, _, cx| {
                                    if let Some(r) = &mut s.runtime {
                                        r.stop();
                                    }
                                    s.message = "已退出".into();
                                    cx.notify();
                                })),
                        )
                        .child(leds),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("speech-panel")
                                .ghost()
                                .small()
                                .label(if speaking {
                                    "播报 · 待处理"
                                } else {
                                    "播报"
                                })
                                .selected(self.settings && self.tab == 2)
                                .on_click(cx.listener(|s, _, _, cx| {
                                    if s.settings && s.tab == 2 {
                                        s.settings = false;
                                    } else {
                                        s.settings = true;
                                        s.tab = 2;
                                    }
                                    s.recording = None;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("auto-reload")
                                .ghost()
                                .small()
                                .selected(self.auto_reload)
                                .label(if self.auto_reload {
                                    "保存后自动重载"
                                } else {
                                    "自动重载已关闭"
                                })
                                .on_click(cx.listener(|s, _, _, cx| {
                                    s.auto_reload = !s.auto_reload;
                                    s.persist_preferences();
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("audio-toggle")
                                .ghost()
                                .small()
                                .label(if self.audio.is_none() {
                                    "无音频设备"
                                } else if self.muted {
                                    "声音已关闭"
                                } else {
                                    "声音开启"
                                })
                                .tooltip(
                                    self.audio_error
                                        .clone()
                                        .unwrap_or_else(|| "切换电脑蜂鸣器声音".into()),
                                )
                                .on_click(cx.listener(|s, _, _, cx| {
                                    s.muted = !s.muted;
                                    s.persist_preferences();
                                    s.refresh();
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("log-toggle")
                                .ghost()
                                .small()
                                .icon(Glyph::Terminal)
                                .selected(self.show_log)
                                .label("事件")
                                .on_click(cx.listener(|s, _, _, cx| {
                                    s.show_log = !s.show_log;
                                    cx.notify();
                                })),
                        ),
                ),
        )
    }
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "--headless") {
        let source = std::fs::read_to_string(args.get(2).ok_or_else(|| {
            anyhow::anyhow!("usage: --headless app.lua output.png [capabilities.json]")
        })?)?;
        let caps = match args.get(4) {
            Some(path) => Capabilities::parse(&std::fs::read_to_string(path)?)?,
            None => Capabilities::mini(),
        };
        let mut runtime = Runtime::new(
            &source,
            "headless",
            caps.clone(),
            Rc::new(RefCell::new(Store::default())),
        )
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        for _ in 0..50 {
            runtime.tick(20);
            std::thread::sleep(Duration::from_millis(20));
        }
        if let Some(e) = &runtime.fault {
            anyhow::bail!("{e}");
        }
        render::render(caps.width(), caps.height(), &runtime.state.borrow().scene)?.save(
            args.get(3)
                .ok_or_else(|| anyhow::anyhow!("output PNG required"))?,
        )?;
        return Ok(());
    }
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(|cx| {
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
            cx.set_quit_mode(QuitMode::LastWindowClosed);
            cx.on_action(|_: &Quit, cx| cx.quit());
            let modifier = if cfg!(target_os = "macos") {
                "cmd"
            } else {
                "ctrl"
            };
            cx.bind_keys([
                KeyBinding::new(&format!("{modifier}-q"), Quit, None),
                KeyBinding::new(&format!("{modifier}-o"), OpenLua, None),
                KeyBinding::new(&format!("{modifier}-r"), ReloadLua, None),
            ]);
            cx.set_menus([
                Menu::new("LingClaw Simulator").items([MenuItem::action("退出", Quit)]),
                Menu::new("文件").items([
                    MenuItem::action("打开 Lua…", OpenLua),
                    MenuItem::action("重载", ReloadLua),
                ]),
            ]);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(960.), px(860.)),
                        cx,
                    ))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("LingClaw Simulator".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    let studio = cx.new(|cx| Studio::new(window, cx));
                    cx.new(|cx| Root::new(studio, window, cx))
                },
            )
            .expect("open native window");
            cx.activate(true);
        });
    Ok(())
}
