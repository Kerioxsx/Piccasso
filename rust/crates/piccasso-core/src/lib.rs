//! Piccasso core library: image processing, local analysis, step generation,
//! rendering, geometry, drawing comparison, lock state, portable project format
//! and storage. Shared by the desktop app and its tests; mirrors the web app's
//! `web/js/core/*` modules (verified by `shared/test-vectors.json`).
pub mod analysis;
pub mod capabilities;
pub mod compare;
pub mod geometry;
pub mod imgproc;
pub mod lock;
pub mod motion;
pub mod pipeline;
pub mod project;
pub mod raster;
pub mod render;
pub mod replica;
pub mod shared;
pub mod steps;
pub mod store;
pub mod strokes;
pub use uuid;

use base64::Engine;

/// Decode a project source (base64 PNG/JPEG/WebP) into RGBA.
pub fn decode_source(src: &project::Source) -> Result<imgproc::Rgba, String> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(src.data.trim()).map_err(|e| format!("Invalid image data: {e}"))?;
    decode_image_bytes(&bytes)
}

pub fn decode_image_bytes(bytes: &[u8]) -> Result<imgproc::Rgba, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("This image could not be opened: {e}"))?.to_rgba8();
    Ok(imgproc::Rgba { width: img.width() as usize, height: img.height() as usize, data: img.into_raw() })
}

/// Encode RGBA as PNG bytes.
pub fn encode_png(img: &imgproc::Rgba) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let buf = image::RgbaImage::from_raw(img.width as u32, img.height as u32, img.data.clone()).ok_or("bad buffer")?;
    buf.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out)
}

/// Build a project source from an image file's bytes, capping the long side at 2048 px.
pub fn source_from_bytes(bytes: &[u8]) -> Result<(project::Source, imgproc::Rgba), String> {
    let mut img = decode_image_bytes(bytes)?;
    let (w, h) = imgproc::fit_dims(img.width, img.height, 2048);
    if (w, h) != (img.width, img.height) {
        img = imgproc::resize_box(&img, w, h);
    }
    let png = encode_png(&img)?;
    let src = project::Source { mime: "image/png".into(), width: img.width as u32, height: img.height as u32, data: base64::engine::general_purpose::STANDARD.encode(png) };
    Ok((src, img))
}
