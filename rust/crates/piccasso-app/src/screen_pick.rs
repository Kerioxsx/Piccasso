//! Picking places on the screen (tools, drawing area) by clicking or dragging.
//! Piccasso goes full-screen over the desktop: on Linux/X11 it shows a fresh
//! screenshot underneath; elsewhere the window is see-through. Positions are
//! read from the real mouse (the same API used for drawing), so they always
//! match the coordinates auto-draw moves to.
use piccasso_core::imgproc::Rgba;

/// Screenshot of the whole X11 screen (Linux). None elsewhere or on failure.
pub fn capture_screen() -> Option<Rgba> {
    #[cfg(all(target_os = "linux", feature = "autodraw"))]
    {
        use x11rb::connection::Connection;
        use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};
        let (conn, n) = x11rb::connect(None).ok()?;
        let screen = &conn.setup().roots[n];
        let (w, h) = (screen.width_in_pixels, screen.height_in_pixels);
        let img = conn.get_image(ImageFormat::Z_PIXMAP, screen.root, 0, 0, w, h, !0).ok()?.reply().ok()?;
        if img.depth < 24 { return None; }
        let (w, h) = (w as usize, h as usize);
        let mut data = vec![255u8; w * h * 4];
        for i in 0..w * h {
            let p = &img.data[i * 4..i * 4 + 4]; // BGRX
            data[i * 4] = p[2]; data[i * 4 + 1] = p[1]; data[i * 4 + 2] = p[0];
        }
        Some(Rgba { width: w, height: h, data })
    }
    #[cfg(not(all(target_os = "linux", feature = "autodraw")))]
    { None }
}

/// Current mouse position in screen pixels (the coordinates auto-draw uses).
pub fn mouse_position() -> Option<(i32, i32)> {
    #[cfg(feature = "autodraw")]
    {
        use enigo::{Enigo, Mouse, Settings};
        Enigo::new(&Settings::default()).ok()?.location().ok()
    }
    #[cfg(not(feature = "autodraw"))]
    { None }
}
