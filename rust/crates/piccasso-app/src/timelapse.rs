//! Timelapse recording: while Piccasso draws, the drawing area of the screen is
//! captured at a steady pace; afterwards the frames become an animated GIF
//! (about 15 seconds long, last frame held) saved in Pictures/Piccasso.
use piccasso_core::imgproc::{resize_box, Rgba};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Longest side of the recorded frames.
const MAX_SIDE: usize = 640;
/// Frames kept at most (older ones are thinned out evenly when exceeded).
const MAX_FRAMES: usize = 400;

pub struct Recorder {
    stop: Arc<AtomicBool>,
    frames: Arc<Mutex<Vec<Rgba>>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

/// Grab only the drawing area (cheap), shrunk to the GIF size.
fn grab(a: [i32; 4]) -> Option<Rgba> {
    let out = crate::screen_pick::capture_region(a)?;
    let (w, h) = (out.width, out.height);
    if w < 2 || h < 2 { return None; }
    let m = w.max(h);
    Some(if m > MAX_SIDE { resize_box(&out, w * MAX_SIDE / m, h * MAX_SIDE / m) } else { out })
}

impl Recorder {
    /// Start capturing `area` (screen pixels: x, y, w, h) every `every`.
    pub fn start(area: [i32; 4], every: Duration) -> Recorder {
        let stop = Arc::new(AtomicBool::new(false));
        let frames = Arc::new(Mutex::new(Vec::new()));
        let (s2, f2) = (stop.clone(), frames.clone());
        let handle = std::thread::spawn(move || {
            while !s2.load(Ordering::Relaxed) {
                let t0 = Instant::now();
                if let Some(f) = grab(area) {
                    let mut v = f2.lock().unwrap();
                    v.push(f);
                    if v.len() > MAX_FRAMES { // keep every other frame
                        let kept: Vec<Rgba> = v.drain(..).enumerate().filter(|(i, _)| i % 2 == 0).map(|(_, f)| f).collect();
                        *v = kept;
                    }
                }
                // a slow screen capture (macOS/Windows) stretches the pace rather than piling up
                let left = every.saturating_sub(t0.elapsed());
                let until = Instant::now() + left.max(Duration::from_millis(50));
                while Instant::now() < until && !s2.load(Ordering::Relaxed) { std::thread::sleep(Duration::from_millis(20)); }
            }
        });
        Recorder { stop, frames, handle: Some(handle) }
    }

    /// Stop, add one last frame, and return all frames.
    pub fn finish(mut self, area: [i32; 4]) -> Vec<Rgba> {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() { let _ = h.join(); }
        let mut v = std::mem::take(&mut *self.frames.lock().unwrap());
        if let Some(f) = grab(area) { v.push(f); }
        v
    }
}

/// Encode frames as a looping GIF of about `seconds`, holding the last frame.
pub fn encode_gif(frames: &[Rgba], seconds: f64, path: &std::path::Path) -> Result<(), String> {
    let first = frames.first().ok_or("Nothing was recorded.")?;
    let (w, h) = (first.width as u16, first.height as u16);
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = gif::Encoder::new(std::io::BufWriter::new(file), w, h, &[]).map_err(|e| e.to_string())?;
    enc.set_repeat(gif::Repeat::Infinite).map_err(|e| e.to_string())?;
    let delay = ((seconds * 100.0 / frames.len() as f64).round() as u16).clamp(4, 50); // centiseconds
    for (i, f) in frames.iter().enumerate() {
        if f.width != first.width || f.height != first.height { continue; }
        let mut px = f.data.clone();
        let mut frame = gif::Frame::from_rgba_speed(w, h, &mut px, 10);
        frame.delay = if i + 1 == frames.len() { 250 } else { delay };
        enc.write_frame(&frame).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Folder for timelapses: Pictures/Piccasso (or the app data folder).
pub fn output_dir(data_dir: &std::path::Path) -> std::path::PathBuf {
    directories::UserDirs::new().and_then(|u| u.picture_dir().map(|p| p.join("Piccasso"))).unwrap_or_else(|| data_dir.join("timelapses"))
}

/// A new file name like `timelapse-20261121-153012.gif`.
pub fn new_path(dir: &std::path::Path) -> std::path::PathBuf {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (days, rem) = (secs / 86400, secs % 86400);
    // civil date from days since 1970-01-01 (UTC)
    let z = days as i64 + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    dir.join(format!("timelapse-{y:04}{m:02}{d:02}-{:02}{:02}{:02}.gif", rem / 3600, rem / 60 % 60, rem % 60))
}

/// Show a file in the system file manager.
pub fn reveal(path: &std::path::Path) {
    let dir = path.parent().unwrap_or(path);
    let cmd = if cfg!(target_os = "windows") { "explorer" } else if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    let _ = std::process::Command::new(cmd).arg(dir).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gif_is_written() {
        let frames: Vec<Rgba> = (0..5).map(|i| Rgba { width: 20, height: 10, data: (0..800).map(|k| ((k * 7 + i * 40) % 255) as u8).collect() }).collect();
        let p = std::env::temp_dir().join("piccasso-test-timelapse.gif");
        encode_gif(&frames, 2.0, &p).unwrap();
        let bytes = std::fs::read(&p).unwrap();
        assert_eq!(&bytes[..6], b"GIF89a");
        let _ = std::fs::remove_file(p);
        assert!(new_path(std::path::Path::new("/x")).to_string_lossy().contains("timelapse-"));
    }
}
