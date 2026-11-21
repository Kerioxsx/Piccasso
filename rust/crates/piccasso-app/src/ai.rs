//! Optional AI step writer (`ai` feature). Raw HTTP to the Claude Messages API
//! (there is no official Rust SDK). Off by default; runs in a background thread;
//! any failure keeps the built-in steps.
use crate::app::PiccassoApp;

#[cfg(feature = "ai")]
mod imp {
    use super::PiccassoApp;
    use base64::Engine;
    use piccasso_core::analysis::Analysis;
    use piccasso_core::imgproc::prepare_image;
    use piccasso_core::project::Step;
    use serde_json::{json, Value};
    use std::sync::mpsc::{channel, Receiver};
    use std::sync::Mutex;

    static RX: Mutex<Option<(String, Receiver<Result<(String, Vec<Value>), String>>)>> = Mutex::new(None);

    const SYSTEM: &str = "You are a patient drawing teacher for complete beginners. You receive a reference picture and an automatic analysis of it (normalized 0-1 coordinates, x to the right, y down). Rewrite each listed tutorial step so it talks about what is actually in THIS picture (name the real objects you see). Rules: plain words, no art jargon without a short explanation, 2-4 short sentences per instruction, never invent details that are not visible in the picture, keep the same step kinds and order, keep tips short and practical.";

    fn request(key: &str, model: &str, jpeg_b64: &str, a: &Analysis, steps: &[Step]) -> Result<(String, Vec<Value>), String> {
        let kinds: Vec<&str> = steps.iter().map(|s| s.kind.as_str()).collect();
        let schema = json!({
            "type": "object", "additionalProperties": false, "required": ["subject", "steps"],
            "properties": {
                "subject": { "type": "string" },
                "steps": { "type": "array", "items": { "type": "object", "additionalProperties": false, "required": ["kind", "title", "instruction", "tip"],
                    "properties": { "kind": { "type": "string", "enum": kinds }, "title": { "type": "string" }, "instruction": { "type": "string" }, "tip": { "type": "string" } } } }
            }
        });
        let summary = json!({ "subjectBox": a.subject.bbox, "subjectAspect": a.subject.aspect, "lowContrast": a.low_contrast,
            "shapes": a.shapes.iter().map(|s| json!({"kind": s.kind, "cx": s.cx, "cy": s.cy, "rx": s.rx, "ry": s.ry})).collect::<Vec<_>>(),
            "lightComesFrom": a.values.light_dir, "darkestArea": a.values.dark_pos, "mostDetail": a.detail.pos });
        let list: String = steps.iter().map(|s| format!("- {}: {}. {}\n", s.kind, s.title, s.instruction)).collect();
        let body = json!({
            "model": model, "max_tokens": 16000, "fallbacks": "default",
            "output_config": { "effort": "medium", "format": { "type": "json_schema", "schema": schema } },
            "system": SYSTEM,
            "messages": [{ "role": "user", "content": [
                { "type": "image", "source": { "type": "base64", "media_type": "image/jpeg", "data": jpeg_b64 } },
                { "type": "text", "text": format!("Analysis:\n{summary}\n\nSteps to rewrite (kind: current text):\n{list}") }
            ] }]
        });
        let mut resp = ureq::post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", key)
            .header("anthropic-version", "2026-10-21")
            .header("anthropic-beta", "server-side-fallback-2026-07-01")
            .header("content-type", "application/json")
            .send_json(&body)
            .map_err(|e| format!("request failed: {e}"))?;
        let v: Value = resp.body_mut().read_json().map_err(|e| format!("bad response: {e}"))?;
        match v["stop_reason"].as_str() {
            Some("refusal") => return Err("the AI service declined this image".into()),
            Some("max_tokens") => return Err("the AI response was cut off".into()),
            _ => {}
        }
        let text: String = v["content"].as_array().map(|a| a.iter().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect()).unwrap_or_default();
        let parsed: Value = serde_json::from_str(&text).map_err(|_| "the AI response could not be read".to_string())?;
        let steps = parsed["steps"].as_array().cloned().ok_or("the AI response was incomplete")?;
        Ok((parsed["subject"].as_str().unwrap_or("").to_string(), steps))
    }

    pub fn maybe_rewrite(app: &mut PiccassoApp) {
        let key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();
        if key.is_empty() { app.error("AI helper is on but ANTHROPIC_API_KEY is not set. Using the built-in steps."); return; }
        let Some(sess) = app.session.as_ref() else { return };
        let Some(a) = sess.project.analysis.clone() else { return };
        let small = prepare_image(&sess.src, &sess.project.prep.image(), 768);
        let mut jpeg = Vec::new();
        let rgb = image::RgbaImage::from_raw(small.width as u32, small.height as u32, small.data.clone()).map(|i| image::DynamicImage::ImageRgba8(i).to_rgb8());
        if let Some(rgb) = rgb { let _ = rgb.write_to(&mut std::io::Cursor::new(&mut jpeg), image::ImageFormat::Jpeg); }
        let b64 = base64::engine::general_purpose::STANDARD.encode(&jpeg);
        let (steps, model, id) = (sess.project.steps.clone(), app.settings.ai_model.clone(), sess.project.id.clone());
        let (tx, rx) = channel();
        std::thread::spawn(move || { let _ = tx.send(request(&key, &model, &b64, &a, &steps)); });
        *RX.lock().unwrap() = Some((id, rx));
        app.toast("Sending a small copy of the picture to the AI helper…");
    }

    pub fn poll(app: &mut PiccassoApp) {
        let mut guard = RX.lock().unwrap();
        let Some((id, rx)) = guard.as_ref() else { return };
        let Ok(res) = rx.try_recv() else { return };
        let id = id.clone();
        *guard = None;
        drop(guard);
        match res {
            Ok((subject, ai_steps)) => {
                if let Some(sess) = app.session.as_mut().filter(|s| s.project.id == id) {
                    for s in sess.project.steps.iter_mut() {
                        if let Some(r) = ai_steps.iter().find(|r| r["kind"] == s.kind.as_str()) {
                            if let Some(instr) = r["instruction"].as_str().filter(|x| !x.is_empty()) {
                                s.local_instruction = Some(s.instruction.clone());
                                s.instruction = instr.into();
                                if let Some(t) = r["title"].as_str() { s.title = t.into(); }
                                if let Some(t) = r["tip"].as_str() { s.tip = t.into(); }
                                s.source = Some("ai".into());
                            }
                        }
                    }
                    sess.project.extra.insert("ai".into(), serde_json::json!({ "used": true, "subject": subject }));
                    app.save_project();
                    app.toast("Personalised steps are ready.");
                }
            }
            Err(e) => app.error(format!("AI helper unavailable: {e}. Using the built-in steps.")),
        }
    }
}

#[cfg(feature = "ai")]
pub use imp::{maybe_rewrite, poll};

#[cfg(not(feature = "ai"))]
pub fn maybe_rewrite(_app: &mut PiccassoApp) {}
#[cfg(not(feature = "ai"))]
pub fn poll(_app: &mut PiccassoApp) {}
