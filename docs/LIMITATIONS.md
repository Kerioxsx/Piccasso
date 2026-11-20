# Limitations

## AR and camera tracing
- The guide is drawn **on the screen** over the camera picture. Nothing is projected onto the paper.
- **WebXR AR** (real surface tracking with anchors) exists mainly in Chrome on ARCore-capable Android phones. It is implemented but has not been verified on a physical device in this project. iPhone/iPad Safari and desktop browsers do not offer WebXR AR; they get the camera overlay.
- The camera overlay has no motion tracking of its own. Corner calibration gives a correct perspective at the moment you set it; if the phone or the paper moves, realign (or use "Follow paper", which re-detects a contrasting sheet a few times per second and holds the last position when it cannot see it).
- Paper detection needs the whole sheet visible on a darker surface and fails on white tables, strong shadows or very busy scenes.
- Phone link (desktop): the phone must be on the same network, and it will warn about the self-signed certificate once. Some browsers refuse camera access on pages with certificate warnings; the page then offers "Send a photo" instead of live video.

## Image analysis
- Analysis is classical computer vision, not object recognition: DrawStep finds regions, edges, tones and colours, so instructions talk about "a large oval near the middle" rather than "the cat's head" (the optional AI helper adds real object names).
- Subject separation assumes the subject differs in colour from the picture's edges. Busy or same-coloured backgrounds may merge with the subject; use crop, object removal, "simplify background" or more contrast. Low-contrast pictures fall back to a whole-picture plan with a notice.
- Pencil-pressure percentages and grades are typical starting points, not measurements of your pencil or paper.

## Auto-draw
- Works where the OS allows synthetic mouse input: Windows, macOS (grant Accessibility permission), Linux X11. Wayland sessions usually block it.
- It draws what DrawStep's strokes describe (outlines and line art), not shading or colour fills, and it relies on the target program's current brush.
- Very high speeds can outrun some programs' input handling; lower the speed if lines look jagged.

## Other
- Browsers decide whether the screen can stay awake (battery saver can refuse). The app shows the status and advises adjusting auto-lock otherwise.
- The desktop app has no built-in text-to-speech; use the operating system's screen reader.
- Native iOS/Android apps were not built; the installable web app is the phone version.
