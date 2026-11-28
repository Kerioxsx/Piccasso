//! Screen capture and the mouse position, in the coordinates auto-draw moves
//! the mouse in. Captures use the operating system directly (X11, GDI,
//! CoreGraphics) at the screen's full resolution: no helper programs, so a
//! capture takes milliseconds and the picker shows a sharp image.
use piccasso_core::imgproc::{resize_box, Rgba};

/// A screenshot at native resolution. `scale` = image pixels per mouse unit
/// (2 on a Retina Mac, 1 elsewhere).
pub struct Shot { pub img: Rgba, pub scale: f64 }

/// The main screen at full resolution.
pub fn capture_native() -> Option<Shot> { native(None) }

/// The main screen in mouse coordinates (for colour detection and measuring).
pub fn capture_screen() -> Option<Rgba> {
    let s = native(None)?;
    Some(to_mouse(s))
}

/// Part of the screen (`[x, y, w, h]` in mouse coordinates), in mouse coordinates. Fast: only that part is read.
pub fn capture_region(r: [i32; 4]) -> Option<Rgba> {
    if r[2] <= 0 || r[3] <= 0 { return None; }
    let s = native(Some(r))?;
    Some(to_mouse(s))
}

fn to_mouse(s: Shot) -> Rgba {
    if (s.scale - 1.0).abs() < 0.01 { return s.img; }
    let (w, h) = (((s.img.width as f64) / s.scale).round().max(1.0) as usize, ((s.img.height as f64) / s.scale).round().max(1.0) as usize);
    resize_box(&s.img, w, h)
}

#[cfg(all(target_os = "linux", feature = "autodraw"))]
fn native(r: Option<[i32; 4]>) -> Option<Shot> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};
    let (conn, n) = x11rb::connect(None).ok()?;
    let screen = &conn.setup().roots[n];
    let (sw, sh) = (screen.width_in_pixels as i32, screen.height_in_pixels as i32);
    let [x, y, w, h] = r.unwrap_or([0, 0, sw, sh]);
    let (x0, y0) = (x.clamp(0, sw - 1), y.clamp(0, sh - 1));
    let (w, h) = ((x + w).min(sw) - x0, (y + h).min(sh) - y0);
    if w <= 0 || h <= 0 { return None; }
    let img = conn.get_image(ImageFormat::Z_PIXMAP, screen.root, x0 as i16, y0 as i16, w as u16, h as u16, !0).ok()?.reply().ok()?;
    if img.depth < 24 { return None; }
    let (w, h) = (w as usize, h as usize);
    let mut data = vec![255u8; w * h * 4];
    for i in 0..w * h {
        let p = &img.data[i * 4..i * 4 + 4]; // BGRX
        data[i * 4] = p[2]; data[i * 4 + 1] = p[1]; data[i * 4 + 2] = p[0];
    }
    Some(Shot { img: Rgba { width: w, height: h, data }, scale: 1.0 })
}

/// GDI screen copy. The app is DPI-aware (set by the window library), so these
/// are physical pixels, the same units the mouse API uses: scale 1, no blur.
#[cfg(all(target_os = "windows", feature = "autodraw"))]
fn native(r: Option<[i32; 4]>) -> Option<Shot> {
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
    unsafe {
        let [x, y, w, h] = r.unwrap_or([0, 0, GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)]);
        if w <= 0 || h <= 0 { return None; }
        let sdc = GetDC(std::ptr::null_mut());
        if sdc.is_null() { return None; }
        let mdc = CreateCompatibleDC(sdc);
        let bmp = CreateCompatibleBitmap(sdc, w, h);
        let old = SelectObject(mdc, bmp);
        let ok = BitBlt(mdc, 0, 0, w, h, sdc, x, y, SRCCOPY | CAPTUREBLT) != 0;
        let mut bi: BITMAPINFO = std::mem::zeroed();
        bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = w;
        bi.bmiHeader.biHeight = -h; // top-down rows
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        bi.bmiHeader.biCompression = BI_RGB;
        let (wu, hu) = (w as usize, h as usize);
        let mut data = vec![0u8; wu * hu * 4];
        let rows = if ok { GetDIBits(mdc, bmp, 0, h as u32, data.as_mut_ptr() as *mut _, &mut bi, DIB_RGB_COLORS) } else { 0 };
        SelectObject(mdc, old);
        DeleteObject(bmp);
        DeleteDC(mdc);
        ReleaseDC(std::ptr::null_mut(), sdc);
        if rows == 0 { return None; }
        for px in data.chunks_exact_mut(4) { px.swap(0, 2); px[3] = 255; } // BGRA -> RGBA
        Some(Shot { img: Rgba { width: wu, height: hu, data }, scale: 1.0 })
    }
}

/// CoreGraphics copy of the main display. Mouse coordinates are points; on
/// Retina screens the image has 2 pixels per point.
#[cfg(all(target_os = "macos", feature = "autodraw"))]
fn native(r: Option<[i32; 4]>) -> Option<Shot> {
    use core_graphics::display::CGDisplay;
    use core_graphics::geometry::{CGPoint, CGRect, CGSize};
    use core_graphics::window::{kCGNullWindowID, kCGWindowImageDefault, kCGWindowListOptionOnScreenOnly};
    let d = CGDisplay::main();
    let b = d.bounds();
    let rect = match r {
        Some([x, y, w, h]) => CGRect::new(&CGPoint::new(x as f64, y as f64), &CGSize::new(w as f64, h as f64)),
        None => b,
    };
    let img = CGDisplay::screenshot(rect, kCGWindowListOptionOnScreenOnly, kCGNullWindowID, kCGWindowImageDefault)?;
    let (w, h, bpr) = (img.width(), img.height(), img.bytes_per_row());
    if w == 0 || h == 0 || img.bits_per_pixel() != 32 { return None; }
    let raw = img.data();
    let bytes = raw.bytes();
    let mut data = vec![255u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let s = y * bpr + x * 4; // BGRA (little-endian, premultiplied first)
            let d = (y * w + x) * 4;
            if s + 3 >= bytes.len() { break; }
            data[d] = bytes[s + 2]; data[d + 1] = bytes[s + 1]; data[d + 2] = bytes[s];
        }
    }
    let scale = (w as f64 / rect.size.width.max(1.0)).max(0.5);
    Some(Shot { img: Rgba { width: w, height: h, data }, scale })
}

#[cfg(not(all(any(target_os = "linux", target_os = "macos", target_os = "windows"), feature = "autodraw")))]
fn native(_r: Option<[i32; 4]>) -> Option<Shot> { None }

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
