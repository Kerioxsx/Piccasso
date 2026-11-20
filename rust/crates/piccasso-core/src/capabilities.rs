//! Camera / AR capability evaluation for the desktop app (pure, testable).
//! Desktop operating systems have no ARKit/ARCore equivalent, so "true AR"
//! (plane tracking) is never reported here; the best mode is a live camera
//! overlay with manual or paper-detection alignment.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayMode {
    /// Live camera feed with overlay (manual / detected paper alignment).
    Camera,
    /// No usable camera: align over a still photo of the paper.
    Photo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraFacts {
    /// Built with the `camera` cargo feature.
    pub camera_feature: bool,
    /// Number of cameras the OS reports (None if enumeration failed).
    pub cameras: Option<usize>,
    /// Last error from opening the camera (permission denied, busy, …).
    pub open_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capability {
    pub mode: OverlayMode,
    pub status: String,
    pub reasons: Vec<String>,
}

pub fn evaluate(f: &CameraFacts) -> Capability {
    let mut reasons = vec!["Plane-tracking AR (ARKit/ARCore) is not available on desktop computers. Use the phone/web version for AR.".to_string()];
    if !f.camera_feature {
        reasons.push("This build was compiled without camera support (enable the `camera` feature).".into());
        return Capability { mode: OverlayMode::Photo, status: "AR unavailable – photo alignment".into(), reasons };
    }
    match f.cameras {
        None => reasons.push("Cameras could not be listed on this system.".into()),
        Some(0) => reasons.push("No camera was found.".into()),
        Some(_) => {}
    }
    if let Some(e) = &f.open_error {
        reasons.push(format!("The camera could not be opened: {e}. Check camera permissions in your system privacy settings."));
    }
    if f.cameras.unwrap_or(0) > 0 && f.open_error.is_none() {
        return Capability { mode: OverlayMode::Camera, status: "Camera overlay (no AR tracking)".into(), reasons };
    }
    Capability { mode: OverlayMode::Photo, status: "AR unavailable – photo alignment".into(), reasons }
}
