use anyhow::{Result, bail};
use std::sync::Mutex;
#[repr(C)]
#[derive(Clone, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub color: u32,
}
#[repr(C)]
#[derive(Clone, Debug)]
pub struct Text {
    pub x: i32,
    pub y: i32,
    pub color: u32,
    pub text: [u8; 64],
}
#[derive(Clone, Default, Debug)]
pub struct Scene {
    pub background: u32,
    pub rects: Vec<Rect>,
    pub texts: Vec<Text>,
}
unsafe extern "C" {
    fn lc_font_init(data: *const u8, len: usize) -> i32;
    fn lc_render(
        w: u32,
        h: u32,
        bg: u32,
        r: *const Rect,
        nr: u32,
        t: *const Text,
        nt: u32,
        rgba: *mut u8,
    ) -> i32;
}
static RENDER_LOCK: Mutex<()> = Mutex::new(());
#[repr(align(8))]
struct AlignedFont([u8; include_bytes!("../assets/lingclaw-16.bin").len()]);
static FONT: AlignedFont = AlignedFont(*include_bytes!("../assets/lingclaw-16.bin"));
pub fn render(w: u32, h: u32, scene: &Scene) -> Result<image::RgbaImage> {
    if w == 0 || h == 0 || w > 2048 || h > 2048 {
        bail!("invalid canvas size");
    }
    let _lock = RENDER_LOCK.lock().unwrap();
    let font = &FONT.0;
    let mut out = image::RgbaImage::new(w, h);
    // LVGL uses process-global state; every call is serialized through this lock.
    let ok = unsafe {
        lc_font_init(font.as_ptr(), font.len()) != 0
            && lc_render(
                w,
                h,
                scene.background,
                scene.rects.as_ptr(),
                scene.rects.len() as u32,
                scene.texts.as_ptr(),
                scene.texts.len() as u32,
                out.as_mut_ptr(),
            ) != 0
    };
    if !ok {
        bail!("renderer initialization failed");
    }
    Ok(out)
}

/// Integer nearest-neighbor expansion with BGRA conversion for GPUI upload.
/// Keep the original RGBA image for screenshots.
pub fn preview_bgra(pixels: &image::RgbaImage, zoom: u32) -> Result<image::RgbaImage> {
    if !matches!(zoom, 1 | 2 | 4)
        || pixels.width() == 0
        || pixels.height() == 0
        || pixels.width() > 2048
        || pixels.height() > 2048
    {
        bail!("unsupported preview dimensions or zoom");
    }
    let row_bytes = pixels.width() as usize * zoom as usize * 4;
    let mut bytes = vec![0; row_bytes * pixels.height() as usize * zoom as usize];
    for (y, source) in pixels
        .as_raw()
        .chunks_exact(pixels.width() as usize * 4)
        .enumerate()
    {
        let start = y * zoom as usize * row_bytes;
        let row = &mut bytes[start..start + row_bytes];
        for (pixel, stretch) in source
            .chunks_exact(4)
            .zip(row.chunks_exact_mut(zoom as usize * 4))
        {
            let bgra = [pixel[2], pixel[1], pixel[0], pixel[3]];
            for destination in stretch.chunks_exact_mut(4) {
                destination.copy_from_slice(&bgra);
            }
        }
        for dy in 1..zoom as usize {
            bytes.copy_within(start..start + row_bytes, start + dy * row_bytes);
        }
    }
    Ok(image::RgbaImage::from_raw(pixels.width() * zoom, pixels.height() * zoom, bytes).unwrap())
}
