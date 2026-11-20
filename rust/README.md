# DrawStep desktop (Rust)

A native Rust app. `drawstep-core` does decoding, image processing, analysis, step generation, motion paths and auto-draw strokes, comparison, the lock state machine, project format and storage. `drawstep-app` is the egui interface plus platform services: keep-awake, file dialogs, webcam, phone bridge (local HTTPS server), mouse control and the optional AI helper.

## Build and run
```bash
cargo run --release -p drawstep-app
```
Cargo features of `drawstep-app`:

| Feature | Default | Adds |
|---|---|---|
| `keepawake` | yes | Keeps the display on in locked mode |
| `autodraw` | yes | Auto-draw with real mouse movement (enigo) |
| `phone` | yes | Phone as camera / AR viewer (tiny_http + rcgen + qrcode) |
| `camera` | no | Webcam via nokhwa |
| `ai` | no | AI step writer via the Claude Messages API (`ANTHROPIC_API_KEY`) |

Linux build/runtime packages: a desktop with X11 or Wayland, `libxkbcommon-x11` (usually installed). Webcam (`camera`) may need `libv4l`/`clang`.

Data folder: the platform data directory (`~/.local/share/drawstep`, `%APPDATA%\DrawStep`, `~/Library/Application Support/app.DrawStep.DrawStep`), or `DRAWSTEP_DATA_DIR`.

Developer flags: `DRAWSTEP_START=<screen>` opens the demo on a screen (`workspace`, `prepare`, `locked`, `overlay`, `compare`, `autodraw`, …) and `DRAWSTEP_STEP=<n>` picks the step. `drawstep --autodraw-test X Y W H [speed]` draws the demo outline into a screen rectangle; `drawstep --serve-phone SECONDS` runs only the phone bridge.

## Test
```bash
cargo test --workspace
```
