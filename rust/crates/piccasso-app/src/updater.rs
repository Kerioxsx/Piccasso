//! In-app updates from the project's GitHub releases.
//!
//! Piccasso asks GitHub for the latest release, compares its version with the
//! running one, and on request downloads the file for this computer, checks
//! its SHA-256 against the digest GitHub publishes, swaps the program in place
//! and restarts. Nothing else is sent: the request carries only a generic
//! user agent. The check can be turned off in Settings.

/// Version of this build. Release builds get it from the workflow
/// (`PICCASSO_VERSION=0.1.<run>`); local builds use the crate version.
pub fn current_version() -> &'static str {
    option_env!("PICCASSO_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"))
}

/// Repository the releases come from (`owner/name`).
pub fn repo() -> String {
    std::env::var("PICCASSO_UPDATE_REPO").unwrap_or_else(|_| "Kerioxsx/Piccasso".into())
}

/// Release file for this operating system and processor.
pub fn asset_name() -> Option<&'static str> {
    if cfg!(target_os = "windows") { Some("Piccasso-windows.zip") }
    else if cfg!(target_os = "macos") { Some("Piccasso-macos.zip") }
    else if cfg!(all(target_os = "linux", target_arch = "x86_64")) { Some("Piccasso-linux.tar.gz") }
    else if cfg!(all(target_os = "linux", target_arch = "aarch64")) { Some("Piccasso-linux-arm64.tar.gz") }
    else { None }
}

/// "v0.1.12" / "0.1.12" -> [0, 1, 12]; anything unparsable compares as 0.
pub fn parse_version(v: &str) -> Vec<u64> {
    v.trim().trim_start_matches(['v', 'V']).split(['.', '-', '+']).take(3).map(|p| p.parse().unwrap_or(0)).collect()
}

/// True when `latest` is a newer version than `current`.
pub fn is_newer(latest: &str, current: &str) -> bool {
    let (mut a, mut b) = (parse_version(latest), parse_version(current));
    a.resize(3, 0);
    b.resize(3, 0);
    a > b
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: String,
    pub url: String,
    pub size: u64,
    /// "sha256:<hex>" when GitHub provides it.
    pub digest: Option<String>,
    pub page: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Idle,
    Checking,
    UpToDate,
    Available(UpdateInfo),
    Downloading(f32),
    /// Installed; restart to use it.
    Ready,
    Failed(String),
}

#[cfg(feature = "update")]
mod net {
    use super::*;
    use std::io::Read;

    fn agent() -> ureq::Agent {
        use ureq::tls::{RootCerts, TlsConfig};
        ureq::Agent::config_builder()
            // the operating system's certificate store (works behind company proxies)
            .tls_config(TlsConfig::builder().root_certs(RootCerts::PlatformVerifier).build())
            .timeout_global(Some(std::time::Duration::from_secs(600)))
            .build()
            .into()
    }

    /// Latest release, if it is newer than this build and has a file for this computer.
    pub fn check() -> Result<Option<UpdateInfo>, String> {
        let Some(name) = asset_name() else { return Err("Updates aren't published for this kind of computer.".into()) };
        let url = format!("https://api.github.com/repos/{}/releases/latest", repo());
        let mut resp = agent().get(&url)
            .header("User-Agent", "piccasso-updater")
            .header("Accept", "application/vnd.github+json")
            .call().map_err(|e| format!("Couldn't reach GitHub: {e}"))?;
        let v: serde_json::Value = resp.body_mut().read_json().map_err(|e| format!("Unexpected answer from GitHub: {e}"))?;
        let tag = v["tag_name"].as_str().unwrap_or_default().to_string();
        if !is_newer(&tag, current_version()) { return Ok(None); }
        let asset = v["assets"].as_array().and_then(|a| a.iter().find(|x| x["name"].as_str() == Some(name)))
            .ok_or_else(|| format!("Version {tag} has no download for this computer yet."))?;
        Ok(Some(UpdateInfo {
            version: tag.trim_start_matches('v').to_string(),
            notes: v["body"].as_str().unwrap_or_default().to_string(),
            url: asset["browser_download_url"].as_str().unwrap_or_default().to_string(),
            size: asset["size"].as_u64().unwrap_or(0),
            digest: asset["digest"].as_str().map(str::to_string),
            page: v["html_url"].as_str().unwrap_or_default().to_string(),
        }))
    }

    fn download(info: &UpdateInfo, progress: &dyn Fn(f32)) -> Result<Vec<u8>, String> {
        let mut resp = agent().get(&info.url).header("User-Agent", "piccasso-updater").call().map_err(|e| format!("Download failed: {e}"))?;
        let mut reader = resp.body_mut().with_config().limit(512 * 1024 * 1024).reader();
        let mut out = Vec::with_capacity(info.size as usize);
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = reader.read(&mut buf).map_err(|e| format!("Download failed: {e}"))?;
            if n == 0 { break; }
            out.extend_from_slice(&buf[..n]);
            if info.size > 0 { progress((out.len() as f64 / info.size as f64).min(1.0) as f32); }
        }
        if let Some(d) = &info.digest {
            use sha2::{Digest, Sha256};
            let want = d.strip_prefix("sha256:").unwrap_or(d).to_ascii_lowercase();
            let got: String = Sha256::digest(&out).iter().map(|b| format!("{b:02x}")).collect();
            if got != want { return Err("The download is damaged (checksum mismatch); nothing was changed.".into()); }
        }
        Ok(out)
    }

    /// The program file inside a Windows zip or Linux tarball.
    fn extract_program(bytes: &[u8]) -> Result<Vec<u8>, String> {
        if cfg!(target_os = "windows") {
            let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
            for i in 0..z.len() {
                let mut f = z.by_index(i).map_err(|e| e.to_string())?;
                if f.name().ends_with("piccasso.exe") { let mut v = Vec::new(); f.read_to_end(&mut v).map_err(|e| e.to_string())?; return Ok(v); }
            }
        } else {
            let mut t = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
            for e in t.entries().map_err(|e| e.to_string())? {
                let mut e = e.map_err(|e| e.to_string())?;
                let path = e.path().map_err(|e| e.to_string())?.to_string_lossy().to_string();
                if path.ends_with("/piccasso") || path == "piccasso" { let mut v = Vec::new(); e.read_to_end(&mut v).map_err(|e| e.to_string())?; return Ok(v); }
            }
        }
        Err("The download doesn't contain the Piccasso program.".into())
    }

    /// The .app bundle this program runs from (macOS), if any.
    pub fn app_bundle() -> Option<std::path::PathBuf> {
        let exe = std::env::current_exe().ok()?;
        let b = exe.parent()?.parent()?.parent()?.to_path_buf(); // X.app/Contents/MacOS/piccasso
        (b.extension().map_or(false, |e| e == "app")).then_some(b)
    }

    /// Replace a macOS bundle with the one in the zip (extracted next to it, then swapped).
    fn replace_bundle(bundle: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
        let parent = bundle.parent().ok_or("no parent folder")?;
        let staging = parent.join(".piccasso-update");
        let _ = std::fs::remove_dir_all(&staging);
        std::fs::create_dir_all(&staging).map_err(|e| format!("Can't write next to the app ({e}). Move Piccasso to Applications or your home folder."))?;
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
        z.extract(&staging).map_err(|e| e.to_string())?; // keeps the executable bit
        let new = staging.join("Piccasso.app");
        if !new.exists() { return Err("The download doesn't contain Piccasso.app.".into()); }
        let old = parent.join(".Piccasso-old.app");
        let _ = std::fs::remove_dir_all(&old);
        std::fs::rename(bundle, &old).map_err(|e| e.to_string())?;
        if let Err(e) = std::fs::rename(&new, bundle) {
            let _ = std::fs::rename(&old, bundle); // put the old one back
            return Err(e.to_string());
        }
        let _ = std::fs::remove_dir_all(&old);
        let _ = std::fs::remove_dir_all(&staging);
        Ok(())
    }

    pub fn install(info: &UpdateInfo, progress: &dyn Fn(f32)) -> Result<(), String> {
        let bytes = download(info, progress)?;
        if cfg!(target_os = "macos") {
            if let Some(b) = app_bundle() { return replace_bundle(&b, &bytes); }
            // a bare binary (built from source): take the one inside the bundle
            let mut z = zip::ZipArchive::new(std::io::Cursor::new(&bytes[..])).map_err(|e| e.to_string())?;
            let mut f = z.by_name("Piccasso.app/Contents/MacOS/piccasso").map_err(|e| e.to_string())?;
            let mut prog = Vec::new();
            f.read_to_end(&mut prog).map_err(|e| e.to_string())?;
            return swap_program(&prog);
        }
        swap_program(&extract_program(&bytes)?)
    }

    fn swap_program(prog: &[u8]) -> Result<(), String> {
        let tmp = std::env::temp_dir().join(format!("piccasso-new-{}", std::process::id()));
        std::fs::write(&tmp, prog).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        { use std::os::unix::fs::PermissionsExt; let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755)); }
        let r = self_replace::self_replace(&tmp).map_err(|e| format!("Couldn't replace the program ({e}). If it's in a protected folder, download the new version from the website."));
        let _ = std::fs::remove_file(&tmp);
        r
    }

    /// Start the new version and quit this one.
    pub fn restart() -> ! {
        if let (true, Some(b)) = (cfg!(target_os = "macos"), app_bundle()) {
            let _ = std::process::Command::new("open").arg("-n").arg(b).spawn();
        } else if let Ok(exe) = std::env::current_exe() {
            let _ = std::process::Command::new(exe).spawn();
        }
        std::process::exit(0);
    }
}

#[cfg(feature = "update")]
pub use net::{check, install, restart};

/// Background update work, polled by the UI every frame.
pub struct Updater {
    pub status: Status,
    rx: Option<std::sync::mpsc::Receiver<Status>>,
}

impl Default for Updater {
    fn default() -> Self { Updater { status: Status::Idle, rx: None } }
}

impl Updater {
    pub fn busy(&self) -> bool { self.rx.is_some() }

    pub fn start_check(&mut self) {
        #[cfg(feature = "update")]
        {
            if self.busy() { return; }
            let (tx, rx) = std::sync::mpsc::channel();
            self.status = Status::Checking;
            std::thread::spawn(move || {
                let s = match check() { Ok(Some(i)) => Status::Available(i), Ok(None) => Status::UpToDate, Err(e) => Status::Failed(e) };
                let _ = tx.send(s);
            });
            self.rx = Some(rx);
        }
    }

    pub fn start_install(&mut self) {
        #[cfg(feature = "update")]
        {
            let Status::Available(info) = self.status.clone() else { return };
            let (tx, rx) = std::sync::mpsc::channel();
            self.status = Status::Downloading(0.0);
            std::thread::spawn(move || {
                let t2 = tx.clone();
                let r = install(&info, &move |f| { let _ = t2.send(Status::Downloading(f)); });
                let _ = tx.send(match r { Ok(()) => Status::Ready, Err(e) => Status::Failed(e) });
            });
            self.rx = Some(rx);
        }
    }

    /// Take messages from the worker; true when something changed.
    pub fn poll(&mut self) -> bool {
        let Some(rx) = &self.rx else { return false };
        let mut changed = false;
        let mut finished = false;
        while let Ok(s) = rx.try_recv() {
            finished = !matches!(s, Status::Downloading(_) | Status::Checking);
            self.status = s;
            changed = true;
        }
        if finished { self.rx = None; }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions_compare_numerically() {
        assert!(is_newer("v0.1.10", "0.1.9"));
        assert!(is_newer("0.2.0", "0.1.99"));
        assert!(!is_newer("v0.1.4", "0.1.4"));
        assert!(!is_newer("v0.1.3", "0.1.4"));
        assert!(!is_newer("nonsense", "0.1.0"));
        assert!(is_newer("v1", "0.9.9"));
    }
}
