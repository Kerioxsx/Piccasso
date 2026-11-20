//! Camera access (optional `camera` feature, via nokhwa) and capability report.
use drawstep_core::capabilities::{evaluate, CameraFacts, Capability};
#[cfg(feature = "camera")]
use drawstep_core::imgproc::Rgba;
use std::sync::OnceLock;

static CAP: OnceLock<Capability> = OnceLock::new();

/// Capability of this computer (computed once).
pub fn capability() -> Capability {
    CAP.get_or_init(|| {
        #[cfg(feature = "camera")]
        {
            let cams = nokhwa::query(nokhwa::utils::ApiBackend::Auto).ok().map(|v| v.len());
            evaluate(&CameraFacts { camera_feature: true, cameras: cams, open_error: None })
        }
        #[cfg(not(feature = "camera"))]
        {
            evaluate(&CameraFacts { camera_feature: false, cameras: None, open_error: None })
        }
    })
    .clone()
}

#[cfg(feature = "camera")]
pub use feed::*;

#[cfg(feature = "camera")]
mod feed {
    use super::Rgba;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    /// A background thread pulling frames from the first camera.
    pub struct CameraFeed {
        pub frame: Arc<Mutex<Option<(u64, Rgba)>>>,
        pub error: Arc<Mutex<Option<String>>>,
        stop: Arc<AtomicBool>,
    }
    impl CameraFeed {
        pub fn start() -> Self {
            let frame = Arc::new(Mutex::new(None));
            let error = Arc::new(Mutex::new(None));
            let stop = Arc::new(AtomicBool::new(false));
            let (f2, e2, s2) = (frame.clone(), error.clone(), stop.clone());
            std::thread::spawn(move || {
                use nokhwa::pixel_format::RgbFormat;
                use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};
                let req = RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestResolution);
                let mut cam = match nokhwa::Camera::new(CameraIndex::Index(0), req) {
                    Ok(c) => c,
                    Err(e) => { *e2.lock().unwrap() = Some(e.to_string()); return; }
                };
                if let Err(e) = cam.open_stream() { *e2.lock().unwrap() = Some(e.to_string()); return; }
                let mut n = 0u64;
                while !s2.load(Ordering::Relaxed) {
                    match cam.frame().and_then(|f| f.decode_image::<RgbFormat>()) {
                        Ok(img) => {
                            let (w, h) = (img.width() as usize, img.height() as usize);
                            let rgb = img.into_raw();
                            let mut data = Vec::with_capacity(w * h * 4);
                            for px in rgb.chunks_exact(3) { data.extend_from_slice(&[px[0], px[1], px[2], 255]); }
                            n += 1;
                            *f2.lock().unwrap() = Some((n, Rgba { width: w, height: h, data }));
                        }
                        Err(e) => { *e2.lock().unwrap() = Some(e.to_string()); break; }
                    }
                }
                let _ = cam.stop_stream();
            });
            CameraFeed { frame, error, stop }
        }
        pub fn latest(&self) -> Option<(u64, Rgba)> { self.frame.lock().ok().and_then(|f| f.clone()) }
        pub fn error(&self) -> Option<String> { self.error.lock().ok().and_then(|e| e.clone()) }
    }
    impl Drop for CameraFeed {
        fn drop(&mut self) { self.stop.store(true, Ordering::Relaxed); }
    }

    /// Grab one webcam frame and start a project from it.
    pub fn capture_still_into_project(app: &mut crate::app::DrawStepApp) {
        let feed = CameraFeed::start();
        let t0 = std::time::Instant::now();
        while t0.elapsed().as_secs() < 5 {
            if let Some((n, img)) = feed.latest() {
                if n > 5 { // let auto-exposure settle
                    match drawstep_core::encode_png(&img).and_then(|b| drawstep_core::source_from_bytes(&b)) {
                        Ok((src, _)) => { crate::screens::home::start_project(app, drawstep_core::project::Project::new("Webcam photo", src), None); }
                        Err(e) => app.error(e),
                    }
                    return;
                }
            }
            if let Some(e) = feed.error() { app.error(format!("The camera could not be opened: {e}. Check camera permissions.")); return; }
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        app.error("The camera did not deliver a picture in time.");
    }
}
