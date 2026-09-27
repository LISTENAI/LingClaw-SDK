use lingclaw_sdk::{
    capabilities::Capabilities,
    render,
    runtime::{Runtime, Store},
};
use std::{cell::RefCell, rc::Rc};
fn store() -> Rc<RefCell<Store>> {
    Rc::new(RefCell::new(Store::default()))
}
fn script(code: &str) -> String {
    format!(
        "function on_tick(dt) end\nfunction on_start() screen.begin(0);screen.present() end\n{code}"
    )
}
#[test]
fn api_versions_remove_unavailable_interfaces() {
    for api in 2..=4 {
        let mut c = Capabilities::mini();
        c.0["runtime"]["lua_sdk"]["version"] = api.into();
        let r = Runtime::new(&script(""), "version", c, store()).unwrap();
        assert_eq!(r.eval::<bool>("return clock ~= nil").unwrap(), api >= 3);
        assert_eq!(
            r.eval::<bool>("return http ~= nil and tts ~= nil and json ~= nil")
                .unwrap(),
            api >= 4
        );
        assert!(
            r.eval::<bool>(
                "return io == nil and os == nil and package == nil and pcall == nil and load == nil"
            )
            .unwrap()
        );
    }
}
#[test]
fn hardware_and_custom_button_ids() {
    let mut c = Capabilities::mini();
    c.0["hardware"]["input"]["buttons"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"id":"left","events":["click"]}));
    c.0["hardware"]["audio"]["speaker"] = false.into();
    c.0["hardware"]["network"] = false.into();
    c.0["hardware"]["lighting"]["leds"] = serde_json::json!([]);
    let mut r = Runtime::new(
        &script("function on_button_click(id) last=id end"),
        "custom",
        c,
        store(),
    )
    .unwrap();
    r.click("left");
    assert_eq!(r.eval::<String>("return last").unwrap(), "left");
    assert!(
        r.eval::<bool>("return led==nil and buzzer==nil and tts==nil and http==nil")
            .unwrap()
    );
}
#[test]
fn infinite_startup_is_bounded() {
    assert!(
        Runtime::new(
            &script("while true do end"),
            "loop",
            Capabilities::mini(),
            store()
        )
        .is_err()
    );
}
#[test]
fn source_and_frame_limits() {
    for code in [
        "function on_tick(dt) end".to_owned(),
        script("screen.text(string.rep('你',22),0,0,1)"),
        script("for i=1,129 do screen.rect(0,0,1,1,1) end"),
    ] {
        assert!(Runtime::new(&code, "limit", Capabilities::mini(), store()).is_err());
    }
}
#[test]
fn storage_survives_restart_and_bad_save_recovers() {
    let data = store();
    Runtime::new(
        &script("assert(storage.save({value=7}))"),
        "save",
        Capabilities::mini(),
        data.clone(),
    )
    .unwrap();
    let r = Runtime::new(
        &script("assert(storage.load().value==7)"),
        "save",
        Capabilities::mini(),
        data.clone(),
    )
    .unwrap();
    drop(r);
    assert!(
        Runtime::new(
            &script("local old=storage.load();error('incompatible')"),
            "save",
            Capabilities::mini(),
            data.clone()
        )
        .is_err()
    );
    Runtime::new(
        &script("assert(storage.load()==nil)"),
        "save",
        Capabilities::mini(),
        data,
    )
    .unwrap();
}
#[test]
fn failed_candidate_does_not_save() {
    let data = store();
    assert!(
        Runtime::new(
            &script("storage.save({v=1});error('fail')"),
            "stage",
            Capabilities::mini(),
            data.clone()
        )
        .is_err()
    );
    Runtime::new(
        &script("assert(storage.load()==nil)"),
        "stage",
        Capabilities::mini(),
        data,
    )
    .unwrap();
}
#[test]
fn http_is_async_and_cancellable() {
    let code = script(
        "result=nil;id=http.get('https://example.com');function on_http_response(i,r) assert(i==id);result=r.status end",
    );
    let mut r = Runtime::with_fixture(
        &code,
        "http",
        Capabilities::mini(),
        store(),
        serde_json::json!({"status":200,"body":"{}","content_type":"application/json"}),
    )
    .unwrap();
    assert!(r.eval::<bool>("return result==nil").unwrap());
    r.tick(20);
    assert_eq!(r.eval::<i64>("return result").unwrap(), 200);
    assert!(r.eval::<bool>("local id=http.get('https://example.com');return http.cancel(id) and not http.cancel(id)").unwrap());
}
#[test]
fn tts_completion_is_explicit() {
    let mut r = Runtime::new(
        &script("function on_button_click() id=tts.speak('你好') end;function on_tts_result(i,r) status=r.status end"),
        "tts",
        Capabilities::mini(),
        store(),
    )
    .unwrap();
    r.click("function");
    r.tick(20);
    assert!(r.eval::<bool>("return status==nil").unwrap());
    r.finish_speech("interrupted");
    r.tick(20);
    assert_eq!(r.eval::<String>("return status").unwrap(), "interrupted");
}
#[test]
fn renderer_keeps_rgb565_and_screen_dimensions() {
    let scene = render::Scene {
        background: 0xff0000,
        ..Default::default()
    };
    let image = render::render(80, 120, &scene).unwrap();
    assert_eq!(image.dimensions(), (80, 120));
    assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0, 255]);
}

#[test]
fn network_parameter_errors_match_device_startup_failures() {
    for code in [
        "http.get('bad-url')",
        "tts.speak('startup')",
        "for i=1,5 do http.get('https://example.com') end",
    ] {
        assert!(Runtime::new(&script(code), "invalid", Capabilities::mini(), store()).is_err());
    }
}

#[test]
fn json_rejects_invalid_values_and_preserves_null_false_arrays() {
    let r = Runtime::new(&script(""), "json", Capabilities::mini(), store()).unwrap();
    for expression in [
        "local t={};t.self=t;return json.encode(t)",
        "return json.encode(0/0)",
        "return json.encode({[1]='a',[3]='b'})",
        "return json.encode({[1]=1,key=2})",
    ] {
        let (value, err): (Option<String>, Option<String>) = r.eval(expression).unwrap();
        assert!(value.is_none() && err.is_some());
    }
    assert_eq!(
        r.eval::<String>("return json.encode(json.decode('[]'))")
            .unwrap(),
        "[]"
    );
    assert!(
        r.eval::<bool>("return json.decode('false')==false and json.decode('null')==json.null")
            .unwrap()
    );
    assert!(
        r.eval::<bool>(r#"local v,e=json.decode('"\\u0000"');return v==nil and e~=nil"#)
            .unwrap()
    );
}

#[test]
fn text_wrapping_overlap_and_clipping_match_golden() {
    let runtime = Runtime::new(
        include_str!("fixtures/render.lua"),
        "render",
        Capabilities::mini(),
        store(),
    )
    .unwrap();
    let got = render::render(240, 240, &runtime.state.borrow().scene).unwrap();
    let expected = image::load_from_memory(include_bytes!("fixtures/render.png"))
        .unwrap()
        .to_rgba8();
    assert_eq!(got, expected);
}
#[test]
fn invalid_dimensions_do_not_wrap_to_a_valid_canvas() {
    let mut c = Capabilities::mini();
    c.0["hardware"]["display"]["width_px"] = 4294967536u64.into();
    assert!(Capabilities::parse(&c.0.to_string()).is_err());
}

#[test]
fn response_can_cancel_another_ready_request() {
    let mut r = Runtime::with_fixture(
        r#"
        local second
        first = http.get("https://example.com/first")
        second = http.get("https://example.com/second")
        calls = 0
        function on_http_response(id, response)
            calls = calls + 1
            assert(id == first)
            assert(http.cancel(second))
        end
        function on_tick() screen.begin(0); screen.present() end
    "#,
        "cancel-ready",
        Capabilities::mini(),
        Rc::new(RefCell::new(Store::default())),
        serde_json::json!({"status":200,"body":"","content_type":""}),
    )
    .unwrap();
    r.tick(20);
    assert!(r.fault.is_none(), "{:?}", r.fault);
    r.eval::<()>("assert(calls == 1)").unwrap();
}

#[test]
fn counter_click_updates_scene_and_pixels() {
    let caps = Capabilities::mini();
    let mut runtime = Runtime::new(
        include_str!("../../examples/counter.lua"),
        "counter",
        caps,
        store(),
    )
    .unwrap();
    let before = render::render(240, 240, &runtime.state.borrow().scene).unwrap();
    runtime.click("function");
    assert!(runtime.fault.is_none());
    assert_eq!(runtime.state.borrow().frames, 2);
    let after = render::render(240, 240, &runtime.state.borrow().scene).unwrap();
    assert_ne!(before, after);
}

#[test]
#[ignore = "manual render latency measurement"]
fn measure_display_work() {
    let mut runtime = Runtime::new(
        include_str!("../../examples/counter.lua"),
        "counter",
        Capabilities::mini(),
        store(),
    )
    .unwrap();
    for zoom in [1, 2, 4] {
        let mut times = Vec::new();
        for _ in 0..40 {
            let start = std::time::Instant::now();
            runtime.click("function");
            let pixels = render::render(240, 240, &runtime.state.borrow().scene).unwrap();
            let bgra = render::preview_bgra(&pixels, zoom).unwrap();
            std::hint::black_box(bgra);
            times.push(start.elapsed().as_micros());
        }
        times.sort();
        eprintln!("{zoom}x: median={} us, p95={} us", times[20], times[38]);
    }
}

#[test]
fn integer_preview_matches_reference_scaling_and_preserves_rgba() {
    let pixels = image::RgbaImage::from_fn(3, 2, |x, y| {
        image::Rgba([x as u8 * 40, y as u8 * 80, 170, 255])
    });
    for zoom in [1, 2, 4] {
        let mut expected = image::imageops::resize(
            &pixels,
            3 * zoom,
            2 * zoom,
            image::imageops::FilterType::Nearest,
        );
        for pixel in expected.pixels_mut() {
            pixel.0.swap(0, 2);
        }
        assert_eq!(render::preview_bgra(&pixels, zoom).unwrap(), expected);
    }
    assert_eq!(pixels.get_pixel(0, 0).0, [0, 0, 170, 255]);
}
