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
```

Both apps include a **demo project** (pear and mug, painted procedurally by `tools/demo-image.mjs`) so everything can be tried without importing a picture. The same file is at `samples/demo-project.piccasso.json`.

More detail: [web/README.md](web/README.md) · [rust/README.md](rust/README.md) · [docs/](docs/).

## Website and downloads (GitHub)

- **Website**: `.github/workflows/pages.yml` publishes `site/` at `https://<owner>.github.io/<repo>/` and the web app at `…/app/` on every push to `main` (enable Pages → "GitHub Actions" in the repository settings once).
- **PC app**: `.github/workflows/release.yml` builds the Rust desktop app for Windows (`Piccasso-windows.zip` with `piccasso.exe`), macOS and Linux on every push that changes `rust/` and publishes a release; the website's download buttons always point at the latest one.

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
11. **Exact replica with shading (desktop)**: choose 2–8 shades and your brush size, record where your program's **brush button** and **opacity control** are (an opacity slider's 0 % and 100 % ends, or one button per shade). For each shade Piccasso clicks the brush, sets the opacity and fills exactly that shade's area with continuous strokes kept one brush-radius inside the boundary; outlines go on last at 100 %. A simulator previews the result and shows the expected match before you start. Without an opacity control, shades are made by stroke spacing. **Depth mode** (for 3D-looking, smoothly shaded pictures) builds tone from many light coats at one opacity, like glazing with a pencil, with a single solid pass for pure black; parts thinner than the fill brush (ink lines, hatching) are traced along their centre with a separate **line brush** you can also record. Paper tint in photos of drawings is treated as white. Tool positions (brush buttons, opacity slider ends, **brush-size slider** ends with their pixel range) and the drawing area can be recorded by hovering for 3 s or with **Pick on screen**: Piccasso shows the screen full-size and you click a tool or drag a box around the canvas. Measured on a test paint program: 97.6 % of pixels within one shade of the picture, average error 2.5/255, 50 s for a 747 × 560 px drawing.
12. **Every detail, smooth motion, colour (desktop)**: ink lines and pencil hatching are found on the full-resolution picture and each is drawn as one smooth stroke at its own darkness and width. The mouse moves continuously, slowing on curves only as much as the program's mouse-reading rate requires, so there are no gaps, no cut corners and no point-by-point jumps. **Colour replica** paints a palette of 2–16 colours (lightest first, no white gaps between them). Piccasso selects each colour in your program by typing it into a **hex field**, or by clicking your **colour wheel, colour square and colour sliders**: it can find them on screen automatically, clicks the closest colour, looks again and repeats until the swatch matches. **Brush-size sliders without pixel numbers** are measured automatically: Piccasso draws a few test lines in an empty spot, measures them on screen and undoes them.
13. **Updates (desktop)**: Piccasso checks GitHub for a new release when it starts (can be turned off in Settings). "Install and restart" downloads the file for your computer, checks its SHA-256 against the one GitHub publishes, replaces the app (the whole Piccasso.app on macOS) and restarts. The web app updates itself in the background and offers "Update now".
14. **Timelapse**: tick "Record a timelapse" in auto-draw to get a 15-second GIF of the drawing (Pictures/Piccasso), or in camera tracing (web) to record your real drawing on paper: a photo every 2 seconds, saved or shared as a video.
15. **Studio (web)**: turn any picture into a drawing reference with classic image processing (pencil sketch, edges, contours, three values, colour → value, posterize, threshold, crosshatching, halftone, silhouette, negative space, background removal, anime simplification, invert), with brightness/contrast, mirror, region-only processing, and PNG / 2× export. No AI anywhere in Piccasso; see `docs/BLUEPRINT.md` for the full feature status.
16. **Guides (web)**: perspective guides (1, 2 or 3 vanishing points you drag, with the horizon), face proportions and 6/7/8-head figure templates you place over the picture, composition grids (thirds, golden ratio, symmetry, diagonals), a printable line-weight chart and a 3× magnifier; save with or without the picture, or print.
17. **Timed practice (web)**: 30-second to 5-minute gesture sessions with your own references, your project, or a built-in mannequin that takes a new natural pose each time (with its line of action); silhouette and mirror modes, pause/resume, and your history and streak.
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
| Audio readout | Done (Web Speech) | Not available (use the OS screen reader) | Button disabled with explanation |
| Native iOS/Android apps | Not built | | The PWA is the phone app |

## Permissions, SDKs and keys

- **Web**: camera (tracing, photo capture) and screen wake lock, both requested only when used. WebXR immersive-ar where available. No accounts, no analytics.
- **Desktop**: file dialogs; camera only with the `camera` feature (macOS/Windows camera permission); local network server only while the phone link is open; mouse control only during auto-draw (macOS: Accessibility permission; Wayland sessions may block synthetic input, use X11/XWayland).
- **No AI**: Piccasso uses no AI models, AI generation or AI image analysis. Every guide comes from deterministic image processing, computer vision and geometry. No API keys or accounts.
- **Libraries** (desktop, permissive licences): eframe/egui, image, serde, rfd, directories, keepawake, enigo (mouse input), tiny_http + rcgen (local HTTPS), qrcode, optional nokhwa (webcam). The web app has no runtime dependencies. The stroke extraction, ordering and drawing logic for auto-draw is Piccasso's own code.

## Privacy

Everything runs on the device by default. Pictures, analysis, projects and comparison photos are never uploaded. The only network use is the desktop phone link, which stays on your local network, and the desktop update check (GitHub releases, can be turned off).

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
