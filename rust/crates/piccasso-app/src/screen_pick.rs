//! Picking places on the screen (tools, drawing area) by clicking or dragging.
//! Piccasso goes full-screen over the desktop: on Linux/X11 it shows a fresh
//! screenshot underneath; elsewhere the window is see-through. Positions are
//! read from the real mouse (the same API used for drawing), so they always
//! match the coordinates auto-draw moves to.
use piccasso_core::imgproc::Rgba;

/// Screenshot of the whole screen in mouse coordinates: X11 on Linux, the
/// `screencapture` tool on macOS, a PowerShell screen copy on Windows. None on failure.
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
    #[cfg(all(any(target_os = "macos", target_os = "windows"), feature = "autodraw"))]
    {
        // the OS screenshot tool writes a PNG; it is scaled to the mouse's coordinate
        // space (Retina / high-DPI screens capture more pixels than mouse points)
        let path = std::env::temp_dir().join(format!("piccasso-shot-{}.png", std::process::id()));
        #[cfg(target_os = "macos")]
        let ok = std::process::Command::new("screencapture").args(["-x", "-t", "png"]).arg(&path).status().map(|s| s.success()).unwrap_or(false);
        #[cfg(target_os = "windows")]
        let ok = {
            let script = format!("Add-Type -AssemblyName System.Windows.Forms,System.Drawing; $b=[System.Windows.Forms.SystemInformation]::VirtualScreen; $bmp=New-Object System.Drawing.Bitmap $b.Width,$b.Height; $g=[System.Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($b.Left,$b.Top,0,0,$bmp.Size); $bmp.Save('{}',[System.Drawing.Imaging.ImageFormat]::Png)", path.display());
            std::process::Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", &script]).status().map(|s| s.success()).unwrap_or(false)
        };
        if !ok { return None; }
        let bytes = std::fs::read(&path).ok()?;
        let _ = std::fs::remove_file(&path);
        let img = piccasso_core::decode_image_bytes(&bytes).ok()?;
        use enigo::{Enigo, Mouse, Settings};
        let (w, h) = Enigo::new(&Settings::default()).ok()?.main_display().ok()?;
        let (w, h) = (w.max(1) as usize, h.max(1) as usize);
        Some(if img.width != w || img.height != h { piccasso_core::imgproc::resize_box(&img, w, h) } else { img })
    }
    #[cfg(not(all(any(target_os = "linux", target_os = "macos", target_os = "windows"), feature = "autodraw")))]
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
