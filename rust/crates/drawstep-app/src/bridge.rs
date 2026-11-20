//! Phone bridge (feature `phone`): use a phone as the desktop app's camera or as
//! an AR viewer. The PC runs a small HTTPS server on the local network with a
//! self-signed certificate (phones only allow camera access on https pages) and
//! a random access token in the URL. Scan the QR code on the phone.
//! Endpoints: `/` phone page · `POST /frame` camera JPEG · `/overlay.png` guide · `/info.json`.
use drawstep_core::imgproc::Rgba;

#[derive(Default)]
pub struct PhoneState {
    #[cfg(feature = "phone")]
    pub bridge: Option<imp::PhoneBridge>,
    pub error: Option<String>,
    pub open: bool,
    pub last_gen: u64,
}

impl PhoneState {
    pub fn running(&self) -> bool {
        #[cfg(feature = "phone")]
        { self.bridge.is_some() }
        #[cfg(not(feature = "phone"))]
        { false }
    }
    pub fn start(&mut self) {
        #[cfg(feature = "phone")]
        match imp::PhoneBridge::start() { Ok(b) => { self.bridge = Some(b); self.error = None; } Err(e) => self.error = Some(e) }
        #[cfg(not(feature = "phone"))]
        { self.error = Some("This build has no phone bridge (enable the `phone` feature).".into()); }
    }
    pub fn stop(&mut self) {
        #[cfg(feature = "phone")]
        { self.bridge = None; }
    }
    pub fn url(&self) -> Option<String> {
        #[cfg(feature = "phone")]
        { self.bridge.as_ref().map(|b| b.url.clone()) }
        #[cfg(not(feature = "phone"))]
        { None }
    }
    pub fn qr(&self) -> Option<Rgba> {
        #[cfg(feature = "phone")]
        { self.bridge.as_ref().map(|b| b.qr.clone()) }
        #[cfg(not(feature = "phone"))]
        { None }
    }
    /// Newest camera frame from the phone (generation, decoded image), if newer than `after`.
    pub fn frame_after(&self, after: u64) -> Option<(u64, Rgba)> {
        #[cfg(feature = "phone")]
        {
            let b = self.bridge.as_ref()?;
            let f = b.frame.lock().ok()?;
            let (g, bytes) = f.as_ref()?;
            if *g <= after { return None; }
            drawstep_core::decode_image_bytes(bytes).ok().map(|img| (*g, img))
        }
        #[cfg(not(feature = "phone"))]
        { let _ = after; None }
    }
    pub fn set_overlay(&self, png: Vec<u8>, info: String) {
        #[cfg(feature = "phone")]
        if let Some(b) = &self.bridge {
            *b.overlay.lock().unwrap() = png;
            *b.info.lock().unwrap() = info;
        }
        #[cfg(not(feature = "phone"))]
        { let _ = (png, info); }
    }
    pub fn stats(&self) -> (u64, bool) {
        #[cfg(feature = "phone")]
        if let Some(b) = &self.bridge {
            let g = b.frame.lock().map(|f| f.as_ref().map(|x| x.0).unwrap_or(0)).unwrap_or(0);
            return (g, *b.viewer_seen.lock().unwrap());
        }
        (0, false)
    }
}

#[cfg(feature = "phone")]
pub mod imp {
    use super::Rgba;
    use std::io::Read;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    pub struct PhoneBridge {
        pub url: String,
        pub qr: Rgba,
        pub frame: Arc<Mutex<Option<(u64, Vec<u8>)>>>,
        pub overlay: Arc<Mutex<Vec<u8>>>,
        pub info: Arc<Mutex<String>>,
        pub viewer_seen: Arc<Mutex<bool>>,
        stop: Arc<AtomicBool>,
    }
    impl Drop for PhoneBridge {
        fn drop(&mut self) { self.stop.store(true, Ordering::Relaxed); }
    }

    /// The computer's address on the local network (no packets are sent).
    fn lan_ip() -> Option<std::net::IpAddr> {
        let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
        s.connect("192.0.2.1:9").ok()?;
        s.local_addr().ok().map(|a| a.ip())
    }

    fn token() -> String {
        drawstep_core::uuid::Uuid::new_v4().simple().to_string()[..16].to_string()
    }

    fn qr_image(text: &str) -> Rgba {
        let code = qrcode::QrCode::new(text.as_bytes()).expect("url fits in a QR code");
        let n = code.width();
        let colors = code.to_colors();
        let (scale, quiet) = (6usize, 4usize);
        let size = (n + 2 * quiet) * scale;
        let mut img = Rgba::filled(size, size, 255);
        for y in 0..n {
            for x in 0..n {
                if colors[y * n + x] == qrcode::Color::Dark {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            let i = (((y + quiet) * scale + dy) * size + (x + quiet) * scale + dx) * 4;
                            img.data[i..i + 3].copy_from_slice(&[0, 0, 0]);
                        }
                    }
                }
            }
        }
        img
    }

    impl PhoneBridge {
        pub fn start() -> Result<Self, String> {
            let ip = lan_ip().ok_or("This computer doesn't seem to be on a network. Connect to Wi-Fi first.")?;
            let names = vec![ip.to_string(), "localhost".to_string()];
            let ck = rcgen::generate_simple_self_signed(names).map_err(|e| format!("certificate: {e}"))?;
            let ssl = tiny_http::SslConfig { certificate: ck.cert.pem().into_bytes(), private_key: ck.key_pair.serialize_pem().into_bytes() };
            let mut server = None;
            let mut port = 0;
            for p in 8765..8790 {
                if let Ok(s) = tiny_http::Server::https(("0.0.0.0", p), ssl.clone()) { server = Some(s); port = p; break; }
            }
            let server = server.ok_or("No free network port between 8765 and 8789.")?;
            let tok = token();
            let url = format!("https://{ip}:{port}/?t={tok}");
            let frame = Arc::new(Mutex::new(None));
            let overlay = Arc::new(Mutex::new(Vec::new()));
            let info = Arc::new(Mutex::new("{}".to_string()));
            let viewer_seen = Arc::new(Mutex::new(false));
            let stop = Arc::new(AtomicBool::new(false));
            let (f2, o2, i2, v2, s2, t2) = (frame.clone(), overlay.clone(), info.clone(), viewer_seen.clone(), stop.clone(), tok.clone());
            std::thread::spawn(move || {
                let mut gen = 0u64;
                while !s2.load(Ordering::Relaxed) {
                    let Ok(Some(mut req)) = server.recv_timeout(Duration::from_millis(200)) else { continue };
                    let raw = req.url().to_string();
                    let (path, query) = raw.split_once('?').unwrap_or((raw.as_str(), ""));
                    let path = path.to_string();
                    let ok_token = query.split('&').any(|kv| kv == format!("t={t2}"));
                    let ct = |v: &str| tiny_http::Header::from_bytes(&b"Content-Type"[..], v.as_bytes()).unwrap();
                    let nocache = tiny_http::Header::from_bytes(&b"Cache-Control"[..], &b"no-store"[..]).unwrap();
                    if !ok_token {
                        let _ = req.respond(tiny_http::Response::from_string("Open the link shown in DrawStep on your computer (it contains an access code).").with_status_code(403));
                        continue;
                    }
                    let resp = match (req.method(), path.as_str()) {
                        (tiny_http::Method::Get, "/") => tiny_http::Response::from_data(PHONE_HTML.replace("__TOKEN__", &t2).into_bytes()).with_header(ct("text/html; charset=utf-8")),
                        (tiny_http::Method::Post, "/frame") => {
                            let mut buf = Vec::new();
                            let _ = req.as_reader().take(6_000_000).read_to_end(&mut buf);
                            if buf.len() > 100 { gen += 1; *f2.lock().unwrap() = Some((gen, buf)); }
                            tiny_http::Response::from_data(b"ok".to_vec())
                        }
                        (tiny_http::Method::Get, "/overlay.png") => { *v2.lock().unwrap() = true; tiny_http::Response::from_data(o2.lock().unwrap().clone()).with_header(ct("image/png")) }
                        (tiny_http::Method::Get, "/info.json") => tiny_http::Response::from_data(i2.lock().unwrap().clone().into_bytes()).with_header(ct("application/json")),
                        _ => tiny_http::Response::from_data(b"not found".to_vec()).with_status_code(404),
                    };
                    let _ = req.respond(resp.with_header(nocache));
                }
            });
            Ok(PhoneBridge { qr: qr_image(&url), url, frame, overlay, info, viewer_seen, stop })
        }
    }

    /// The page served to the phone (self-contained, no external requests).
    pub const PHONE_HTML: &str = include_str!("phone.html");
}
