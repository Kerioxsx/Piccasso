# Limitations

## AR and camera tracing
- The guide is drawn **on the screen** over the camera picture. Nothing is projected onto the paper.
- **WebXR AR** (real surface tracking with anchors) exists mainly in Chrome on ARCore-capable Android phones. It is implemented but has not been verified on a physical device in this project. iPhone/iPad Safari and desktop browsers do not offer WebXR AR; they get the camera overlay.
- The camera overlay has no motion tracking of its own. Corner calibration gives a correct perspective at the moment you set it; if the phone or the paper moves, realign (or use "Follow paper", which re-detects a contrasting sheet a few times per second and holds the last position when it cannot see it).
- Paper detection needs the whole sheet visible on a darker surface and fails on white tables, strong shadows or very busy scenes.
- Phone link (desktop): the phone must be on the same network, and it will warn about the self-signed certificate once. Some browsers refuse camera access on pages with certificate warnings; the page then offers "Send a photo" instead of live video.

## Image analysis
- Analysis is classical computer vision, not object recognition: Piccasso finds regions, edges, tones and colours, so instructions talk about "a large oval near the middle" rather than "the cat's head" (by design: Piccasso uses no AI).
- Subject separation assumes the subject differs in colour from the picture's edges. Busy or same-coloured backgrounds may merge with the subject; use crop, object removal, "simplify background" or more contrast. Low-contrast pictures fall back to a whole-picture plan with a notice.
- Pencil-pressure percentages and grades are typical starting points, not measurements of your pencil or paper.

## Auto-draw
- Works in any program that accepts the system mouse (it moves the real cursor, like your hand would): paint programs, image editors, browser whiteboards, remote desktops. The OS must allow synthetic input: Windows, macOS (grant Accessibility; Screen Recording for colour detection), Linux X11. Wayland sessions usually block it.
- Exceptions on Windows: a program running as administrator ignores input from a normal program (run Piccasso as administrator too); some games and anti-cheat software block synthetic input entirely.
- Every tool position is optional. With none recorded, Piccasso draws with the program's current brush and colour, and makes shades by stroke spacing.
- It draws what Piccasso's strokes describe (outlines and line art), not shading or colour fills, and it relies on the target program's current brush.
- Drawing programs read the mouse 60–240 times a second and join the readings with straight lines. Piccasso moves the mouse continuously and plans the speed so curves stay within half a pixel at the rate you set ("Program reads mouse", 60 by default); real corners get one reading-length pause. If curves look flattened, lower that number.
- Colour replica uses a small palette (2–16 opaque paints), not a continuous blend of colours. Colour selection by clicking wheels and sliders reads the screen: this works on Linux/X11; on macOS (`screencapture`, needs Screen Recording permission) and Windows (PowerShell screen copy) it is implemented but untested, and display scaling other than 100 % can offset the clicks. The hex field mode needs no screen reading.
- Auto-detection of colour tools looks for smooth gradients. Check the list it finds: a photo or a gradient image open on screen can be mistaken for a picker. Pickers drawn with hard steps (palettes of swatches) are not detected; add them by hand.
- Brush-size measuring draws test lines in the spot you choose and removes them with Ctrl+Z (Cmd+Z on macOS); programs with a different undo key need the test lines removed by hand. Soft or textured brushes measure a little wider than their core.
- Depth mode stacks coats, so it needs a program whose opacity applies per stroke and multiplies when strokes are layered (most paint programs at 100 % flow). Detail finer than the line brush is lost, and drawing a detailed picture takes several minutes.
- Exact replica: the copy is exact up to the number of shades you choose (2–8) and the brush you set. Set the same brush size in your program, 100 % hardness, and per-stroke opacity (not flow/build-up). Soft brushes, pressure dynamics, smoothing/stabilisers or "flow" accumulation change the result. Opacity is set by clicking the recorded slider position, so the program must accept a click on its slider (most do; some need a typed value, then use "a button per shade" with presets).

## Updates and timelapse
- The desktop updater replaces the program where it is. If Piccasso sits in a folder you can't write to (for example Program Files without administrator rights), download the new version from the website instead. On macOS the whole Piccasso.app is swapped; the app is ad-hoc signed, not notarised, so macOS may ask again on first start.
- Desktop timelapses capture the drawing area of the screen twice a second (slower on macOS/Windows, where each capture runs the system screenshot tool). Windows and macOS capture are untested.
- Web timelapses need a browser that can record video from a canvas (MediaRecorder); frames are kept in memory until saved, so very long sessions are thinned out (at most 900 photos).

## Other
- Browsers decide whether the screen can stay awake (battery saver can refuse). The app shows the status and advises adjusting auto-lock otherwise.
- The desktop app has no built-in text-to-speech; use the operating system's screen reader.
- Native iOS/Android apps were not built; the installable web app is the phone version.
