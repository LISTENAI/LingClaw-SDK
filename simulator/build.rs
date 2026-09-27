use std::{fs, path::Path};
fn sources(path: &Path, build: &mut cc::Build) {
    for item in fs::read_dir(path).unwrap() {
        let p = item.unwrap().path();
        if p.is_dir() {
            sources(&p, build);
        } else if p.extension().is_some_and(|s| s == "c") {
            build.file(p);
        }
    }
}
fn main() {
    assert!(
        Path::new("vendor/lvgl/lvgl.h").exists(),
        "Run git submodule update --init --recursive"
    );
    println!("cargo:rerun-if-changed=native");
    println!("cargo:rerun-if-changed=vendor/lvgl");
    let mut build = cc::Build::new();
    build
        .include("native")
        .include("vendor/lvgl")
        .define("LV_CONF_INCLUDE_SIMPLE", None)
        .warnings(false);
    sources(Path::new("vendor/lvgl/src"), &mut build);
    build.file("native/renderer.c").compile("lingclaw_render");
}
