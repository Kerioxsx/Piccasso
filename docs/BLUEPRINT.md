# Piccasso feature blueprint: status

Piccasso teaches drawing; it does not draw for you. Every feature uses deterministic image processing, computer vision and geometry. **No AI**: no AI generation, no AI tutors, no AI image analysis, no AI-written instructions (the old optional AI helper was removed).

Legend: ✅ done · 🟡 partly done · ⬜ to build. "Web" = installable web app (phone/tablet/browser), "Desktop" = Rust app.

## 1. Drawing assistance and guidance
| Feature | Status | Where |
|---|---|---|
| Tracing overlay with adjustable transparency | ✅ | Web camera tracing, desktop overlay |
| Reference lock | ✅ | Locked reference (both) |
| Progressive drawing layers (rough → details) | ✅ | Guided steps (both) |
| Shape construction | ✅ | "Basic shapes" step |
| Grid drawing mode (custom grids) | ✅ | Prepare → grid (cm), guide layers |
| Contour mode | ✅ | Contour step/layer, Studio → Contours |
| Negative space mode | ✅ | Studio → Negative space |
| Perspective guide (vanishing points) | ✅ | **Guides** → 1/2/3-point, draggable vanishing points and horizon |
| Proportion guides (face, figure) | ✅ | Guides → face canon, 6/7/8-head figure, composition (thirds, golden ratio, symmetry, diagonals) |
| Mirror mode | ✅ | Prepare (flip), Studio → Mirror |
| Smart zoom (magnify, keep alignment) | ✅ | Locked view pinch zoom; Guides magnifier (3× loupe) |
| Crop to paper / paper calibration | ✅ | Prepare (paper sizes in cm, bank-card measure), AR corner calibration |
| Line weight guide | ✅ | Pencil-pressure step; Guides → printable line-weight chart |
| Gesture drawing mode | ✅ | Practice → gesture timers with mannequin or your own references |
| Step rewind / checkpoints / hints / beginner mode | ✅ | Steps with back/next, saved progress, tips, glossary |

## 2. Image conversion and processing
| Feature | Status | Where |
|---|---|---|
| Pencil sketch, outline, contours, grayscale, three-level shading, crosshatching, posterize, halftone, threshold, invert, silhouette, negative space, edge detection, colour-to-value | ✅ | **Studio** (web) |
| Background removal (conventional segmentation) | ✅ | Studio → Remove background |
| Anime simplification (preset filter) | ✅ | Studio → Anime simplify |
| Adjustable detail, contrast, brightness, threshold, line thickness | ✅ | Studio sliders |
| Selective region processing | ✅ | Studio → Convert only a region |
| Custom cropping | ✅ | Prepare |
| Resolution enhancement (interpolation) | ✅ | Studio → Save 2× (bicubic) |
| Basic shape conversion, portrait construction, perspective line extraction | 🟡 | Shapes step exists; dedicated conversions planned |
| Studio in the desktop app | ✅ | Rust port (`piccasso-core/src/filters.rs`), same conversions |

## 3. AR camera drawing
| Feature | Status |
|---|---|
| Live overlay, paper boundary detection, perspective correction, anchoring, transparency, rotation, position calibration, manual anchors (4 corners), paper follow | ✅ (web) |
| WebXR plane detection + anchors | ✅ where the browser supports it |
| AR grid / construction shapes / contours overlays | ✅ (layer chips) |
| Timelapse of the real drawing | ✅ |
| Camera zoom, freeze frame, front/rear switch, focus/lighting hints | ✅ web (zoom 100–400 % magnifies picture and guide together; freeze holds the frame; camera switch where two cameras exist; hints for dark, washed-out, glare, uneven light and blur from brightness statistics and Laplacian sharpness). Desktop: zoom, freeze and hints on the phone-camera or webcam picture |
| Hand detection | ✅ classical YCbCr skin-colour mask with speckle removal; “Show my hand in front of the guide” draws your hand over the guide (web and desktop). No AI |

## 4. Workspace
| Feature | Status |
|---|---|
| Minimal interface, fullscreen, touch controls, side-by-side comparison, session timer, undo (step history) | ✅ |
| Themes: match device, light, dark, **paper** (warm paper with a fine grain) and **graphite**; accent colour (6 presets or any colour); interface size | ✅ |
| Floating toolbar, custom tool placement, keyboard shortcut editor, workspace presets, multiple references, split screen | ⬜ |

## 5. Learning system (skill tree, lessons, exercises)
| Feature | Status |
|---|---|
| Glossary, technique explanations inside steps, pressure/shading/colour guides | ✅ |
| **Skill tree** with the 10 categories (basic shapes → advanced techniques), 30 lessons in three difficulty levels, unlocked in order | ✅ |
| Step-by-step lessons with a goal, checkable steps and a button that opens the matching practice tool (Guides, 3D, Practice, Studio, a new project) | ✅ |
| Completed-lesson tracking, learning history, milestones, bookmarks ("my practice set"), personal notes, offline (all on the device) | ✅ |
| Visual demonstrations (animated) and more lessons per category | ⬜ |

## 6. Drawing analysis (no AI)
| Feature | Status |
|---|---|
| Reference vs drawing comparison: side by side, opacity, difference overlay, alignment | ✅ (Check photo) |
| Grayscale comparison | ✅ |
| **Analyze**: value histograms of both, brightness distribution, contrast, darkest/lightest values, dark/mid/light shares, and rule-based advice ("your darks are too light…") | ✅ |
| Symmetry check (mirror line, difference heat map, score), matching grids with cell labels, measurements in % of width and cm with ratios, colour and value sampler, annotation pins with notes | ✅ |

## 7. 3D construction sandbox
✅ **3D** screen: cube, sphere, cylinder, cone, pyramid; any combination; move, turn, tilt and scale each shape; orbit camera (drag), zoom (wheel/pinch); perspective with adjustable lens or orthographic; vanishing points and horizon of the selected shape; movable light with flat shading and ground shadows; solid, wireframe and silhouette views; construction lines (each shape's axes); ground grid; scenes saved on the device; PNG screenshots. Rendering is plain canvas geometry (no WebGL needed).

## 8. Personal library
| Feature | Status |
|---|---|
| Project library, thumbnails, recent, automatic local saving, offline access, import/export, backup | ✅ |
| Folders, tags, favourites, project notes, search across names/tags/notes, sorting (recent, name, least finished) | ✅ (kept in the project file, so they travel with exports) |
| Custom covers, version history, before/after | ✅ web: dated photos of your drawing with notes (up to 40, the first is always kept), before/after slider, cover from a photo, the reference or any picture. Stored in the project file, so the desktop app keeps them; desktop viewer ⬜ |

## 9. Customization
Overlay opacity, grid spacing, paper dimensions, text scale, interface size, motion, themes and accent colours ✅; guide colour/thickness in Guides ✅. Keyboard shortcut editor, custom toolbars, templates ⬜.

## 10. Advanced drawing modes
Style presets exist (sketch, realistic, anime, etc.) 🟡. Dedicated portrait / landscape / still life / figure / architecture / manga / technical / symmetry modes ⬜.

## 11. Gesture and timed practice
✅ **Practice** screen: 30 s / 1 min / 2 min / 5 min presets, a 30 s → 5 min warm-up ladder and custom timers; random reference rotation; your own pose collection (stored on the device), the current project, or a built-in procedural mannequin in random natural poses with its line of action; silhouette and mirror practice; pause/resume, next, keyboard shortcuts; session history and statistics (total and weekly minutes, poses, day streak). ⬜ Bundled anatomy reference library.

## 12. Export and sharing
Guides, steps, project files, print/PDF guide, images, timelapses ✅. Studio PNG and 2× export ✅. Progress comparison export, print-ready paper templates 🟡.

## 13. Platform
Web: HTML/CSS/JS, responsive, canvas, camera, IndexedDB, offline service worker, PWA ✅; WebGL for the 3D sandbox ⬜.
Desktop: Rust core shared with the web algorithms, native performance, multithreaded processing, native files, Windows/macOS (universal)/Linux x64+ARM, in-app updates ✅; GPU filters ⬜.
Privacy: local-first, no account, no data collection, camera permission explained ✅; optional cloud sync ⬜ (would be opt-in).

## Build order (next)
1. ~~Perspective and proportion guide overlays; line-weight chart~~ (done: Guides).
2. ~~Gesture and timed practice~~ (done: Practice).
3. ~~3D construction sandbox~~ (done: 3D).
4. ~~Skill tree, lessons and exercises~~ (done: Learn → Skill tree).
5. ~~Analysis tools~~ (done: Analyze).
6. ~~Workspace themes and library organisation~~ (done; version history still open).
7. ~~Desktop ports~~: Studio, Guides, Practice, 3D sandbox, Skill tree and Analyze are in the desktop app too (Rust ports in `piccasso-core`: filters, guides, practice, sandbox3d, inspect).
