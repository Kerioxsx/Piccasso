# Piccasso desktop (Rust)

A native Rust app. `piccasso-core` does decoding, image processing, analysis, step generation, motion paths and auto-draw strokes, comparison, the lock state machine, project format and storage. `piccasso-app` is the egui interface plus platform services: keep-awake, file dialogs, webcam, phone bridge (local HTTPS server), mouse control and the optional AI helper.

## Build and run
```bash
cargo run --release -p piccasso-app
```
Cargo features of `piccasso-app`:

| Feature | Default | Adds |
|---|---|---|
| `keepawake` | yes | Keeps the display on in locked mode |
| `autodraw` | yes | Auto-draw with real mouse movement (enigo) |
| `phone` | yes | Phone as camera / AR viewer (tiny_http + rcgen + qrcode) |
| `camera` | no | Webcam via nokhwa |
| `ai` | no | AI step writer via the Claude Messages API (`ANTHROPIC_API_KEY`) |

Linux build/runtime packages: a desktop with X11 or Wayland, `libxkbcommon-x11` (usually installed). Webcam (`camera`) may need `libv4l`/`clang`.

Data folder: the platform data directory (`~/.local/share/piccasso`, `%APPDATA%\Piccasso`, `~/Library/Application Support/app.Piccasso.Piccasso`), or `PICCASSO_DATA_DIR`.

Developer flags: `PICCASSO_START=<screen>` opens the demo on a screen (`workspace`, `prepare`, `locked`, `overlay`, `compare`, `autodraw`, …) and `PICCASSO_STEP=<n>` picks the step. `piccasso --autodraw-test X Y W H [speed]` draws the demo outline into a screen rectangle; `piccasso --replica-test …` draws a full replica (see `main.rs`; env `PICCASSO_COLORS`, `PICCASSO_DETECT`, `PICCASSO_HEX`, `PICCASSO_COLOR_AREAS`, `PICCASSO_SWATCH`, `PICCASSO_CALIBRATE`); `cargo run -p piccasso-core --example detect_tools -- shot.png` lists the colour pickers found on a screenshot; `piccasso --serve-phone SECONDS` runs only the phone bridge.

## Test
```bash
cargo test --workspace
```
