# Piccasso

_Formerly DrawStep. Projects saved as `.drawstep.json` still open in both apps._

Piccasso is a beginner-friendly drawing coach. Pick any picture and it turns it into simple, image-specific steps you follow on **real paper**: basic shapes, proportions, outline, pencil pressure, shading and colour. It keeps the guide steady in a **locked reference** screen, overlays it on your paper through the **camera**, and on the desktop it can even **auto-draw** the outline in another drawing program by moving your real mouse.

It ships as two separate, working implementations that share one project format:

| | Web app (`web/`) | Desktop app (`rust/`) |
|---|---|---|
| Tech | HTML5 + CSS3 + vanilla JavaScript (ES modules), PWA, no build step | Rust + egui; all processing, storage and platform services in Rust |
| Runs on | Phones, tablets and desktop browsers; installable, works offline | Windows, macOS, Linux |
| Engine | `web/js/core/*` | `rust/crates/piccasso-core` (bit-exact port, verified by shared test vectors) |

The marketing website lives in `site/` (sketch-paper design, real recordings of the app).

## Quick start

**Web app** (needs any static server; camera/AR need `https://` or `localhost`):

```bash
python3 -m http.server 8080 -d web      # then open http://localhost:8080
```

**Desktop app** (Rust 1.80+):

```bash
cd rust
cargo run --release -p piccasso-app                    # auto-draw + phone link included
cargo run --release -p piccasso-app --features camera  # + webcam (nokhwa)
cargo run --release -p piccasso-app --features ai      # + optional AI step writer
```

Both apps include a **demo project** (pear and mug, painted procedurally by `tools/demo-image.mjs`) so everything can be tried without importing a picture. The same file is at `samples/demo-project.piccasso.json`.

More detail: [web/README.md](web/README.md) · [rust/README.md](rust/README.md) · [docs/](docs/).

## What it does

1. **Import** a picture (file, camera capture, or the demo); crop with aspect presets (free, original, square, 3:4, 4:3, paper), rotate, mirror, flip; brightness and contrast.
2. **Choose a style**: original, clean outline, simplified outline, pencil sketch, light sketch, contour only, shading guide, basic shapes. **Difficulty**: Very Easy, Easy, Intermediate, Detailed. **Medium**: one pencil, or colours.
3. **Paper**: A4, A5, Letter, sketchbook, square, or any **custom size in cm**; orientation; border; optional **grid method** (Piccasso calculates the drawing size and the number of squares). The web app also has an on-screen ruler calibrated with a bank card.
4. **Analysis** (on device): subject and objects, construction shapes (ovals, circles, boxes), silhouette and inner contours, value bands, light direction, where detail concentrates, landmarks, **pencil-pressure bands** and a **colour palette**. You can remove or isolate objects and simplify the background.
5. **Guided steps** written for *this* picture, up to 11 kinds: page, shapes, proportions, outline, inner forms, features, clean-up, **pencil pressure & grades** (e.g. "light, 15% pressure, HB… firm, 75%, 4B"), shadows & values (with a **shading method** per level: smooth strokes, hatching, crosshatching and blending), **colour** (palette by name, light to dark layering), details. Each step has a tip, a **hand-movement tip**, a self-check list, glossary links and an overlay.
6. **Show me the motion**: an animated pencil traces each stroke of the step in order and direction (ovals from the top, outlines from their highest point, hatching strokes for shading steps).
7. **Locked reference**: interaction lock (pan/zoom/scroll/tap blocked), press-and-hold unlock, screen kept awake, brightness/contrast/opacity, fit or custom view, timer, one-tap back to step.
8. **Camera tracing**: live camera with transparent overlay, opacity/size/rotation/position, **4-corner paper calibration in true perspective**, automatic paper detection, optional "follow paper" tracking, alignment lock, layer toggles. **WebXR AR** (plane hit-test + anchors) on supported phones. Desktop: webcam, a photo, or **your phone as the camera** (see below).
9. **Phone link (desktop)**: scan a QR code; the phone either streams its camera to the PC or shows the PC's guide over the phone camera (AR viewer). Local HTTPS with a self-signed certificate and a random access token.
10. **Auto-draw (desktop)**: pick a drawing source (outline, outline + inner shapes, or detailed line art), mark the canvas corners of any drawing program, and Piccasso moves the real mouse to draw it, fast, with optimised stroke order. Moving the mouse stops it instantly.
11. **Exact replica with shading (desktop)**: choose 2–8 shades and your brush size, record where your program's **brush button** and **opacity control** are (an opacity slider's 0 % and 100 % ends, or one button per shade). For each shade Piccasso clicks the brush, sets the opacity and fills exactly that shade's area with continuous strokes kept one brush-radius inside the boundary; outlines go on last at 100 %. A simulator previews the result and shows the expected match before you start. Without an opacity control, shades are made by stroke spacing. Measured on a test paint program: 97.6 % of pixels within one shade of the picture, average error 2.5/255, 50 s for a 747 × 560 px drawing.
12. **Check a photo** of your drawing (on device, never uploaded): paper detection, flattening, gentle structural feedback ("about 12% wider than the guide").
13. **Projects** saved locally (IndexedDB / project files on disk), rename, duplicate, delete, resume; export guide PNG, printable step sheet, portable project file; import from either app.

## Implementation status

| Feature | Web | Desktop | Notes |
|---|---|---|---|
| Import, crop, rotate, flip, adjust | Done | Done | HEIC only where the browser decodes it |
| 8 styles, 4 difficulty levels | Done | Done | Identical pixels in both (conformance tests) |
| Object remove/isolate, background simplification | Done | Done | |
| Image-specific steps, glossary, checks | Done | Done | Shared text templates `shared/strings.en.json` |
| Pressure, grades, shading methods, palette | Done | Done | |
| Hand-motion animation | Done | Done | |
| Custom paper (cm), grid method, size calculator | Done | Done | Card-calibrated ruler: web only |
| Locked reference + keep-awake | Done | Done | Web: Screen Wake Lock API; desktop: OS API (`keepawake`) |
| Camera overlay, corner calibration, paper detection | Done | Done (webcam needs `camera` feature) | |
| WebXR AR (plane detection, anchors) | Implemented, **untested on a physical device** | n/a | Desktop OSes have no ARKit/ARCore |
| Phone as camera / AR viewer for the PC | n/a | Done | Tested with an emulated phone + fake camera |
| Auto-draw with real mouse movement | Not possible in browsers | Done | Tested on X11 with a browser paint pad |
| Exact replica (shades, opacity/brush tools) | Not possible in browsers | Done | Tested end to end on X11 with a mock paint program |
| Photo comparison | Done | Done | |
| Projects, export/import, step sheet | Done | Done | Step sheet: printable HTML |
| Optional AI step writer (Claude) | Done | Done (`ai` feature) | Off by default; your own API key; explicit consent |
| Audio readout | Done (Web Speech) | Not available (use the OS screen reader) | Button disabled with explanation |
| Native iOS/Android apps | Not built | | The PWA is the phone app |

## Permissions, SDKs and keys

- **Web**: camera (tracing, photo capture) and screen wake lock, both requested only when used. WebXR immersive-ar where available. No accounts, no analytics.
- **Desktop**: file dialogs; camera only with the `camera` feature (macOS/Windows camera permission); local network server only while the phone link is open; mouse control only during auto-draw (macOS: Accessibility permission; Wayland sessions may block synthetic input, use X11/XWayland).
- **API keys**: none required. The optional AI helper uses an Anthropic API key (web: entered in Settings, kept for the session unless you choose to remember it; desktop: `ANTHROPIC_API_KEY`). Model default `claude-opus-5-5`.
- **Libraries** (desktop, permissive licences): eframe/egui, image, serde, rfd, directories, keepawake, enigo (mouse input), tiny_http + rcgen (local HTTPS), qrcode, optional nokhwa (webcam) and ureq (AI). The web app has no runtime dependencies (the AI helper loads the Anthropic SDK from a CDN only when used). The stroke extraction, ordering and drawing logic for auto-draw is Piccasso's own code.

## Privacy

Everything runs on the device by default. Pictures, analysis, projects and comparison photos are never uploaded. The only network use is the optional AI helper (after you turn it on and confirm) and the desktop phone link, which stays on your local network.

## Tests

```bash
npm test                      # JS unit tests (core engine, project format, storage, lock, AR capability, technique)
npm run test:e2e              # browser smoke test (Playwright/Chromium)
cd rust && cargo test         # Rust unit tests + cross-implementation conformance vectors
```

`shared/test-vectors.json` is generated by the JS engine (`node tools/make-vectors.mjs`) and the Rust tests must reproduce it exactly: pixel hashes for every style, contours, analysis JSON and step text.

## Repository layout

```
shared/        Declarative data used by both apps (difficulty, styles, paper, step kinds, strings, glossary, pressure, colours) + test vectors
schema/        JSON Schema of the portable project format v1
samples/       Demo project and image
web/           Web app (PWA): js/core = engine, js/ui = screens, tests/
rust/          Cargo workspace: piccasso-core (engine, storage) + piccasso-app (egui desktop app)
site/          Marketing website (index.html + media recorded from the real app)
tools/         Data generators, demo painter, icon renderer, video recorder
docs/          Architecture, algorithms, project format, limitations
```

See [docs/LIMITATIONS.md](docs/LIMITATIONS.md) for what AR tracking and image analysis can and cannot do.
