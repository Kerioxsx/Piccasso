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
| Gesture drawing mode | ⬜ | planned with timed practice (§11) |
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
| Studio in the desktop app | ⬜ | planned (core filters to port to Rust) |

## 3. AR camera drawing
| Feature | Status |
|---|---|
| Live overlay, paper boundary detection, perspective correction, anchoring, transparency, rotation, position calibration, manual anchors (4 corners), paper follow | ✅ (web) |
| WebXR plane detection + anchors | ✅ where the browser supports it |
| AR grid / construction shapes / contours overlays | ✅ (layer chips) |
| Timelapse of the real drawing | ✅ |
| Camera zoom, freeze frame, front/rear switch, focus/lighting hints | ⬜ |
| Hand detection | ⬜ (classical skin/motion detection planned; no AI) |

## 4. Workspace
| Feature | Status |
|---|---|
| Minimal interface, fullscreen, dark/light themes, interface scaling, touch controls, side-by-side comparison, session timer, undo (step history) | ✅ |
| Paper and graphite themes, accent colours, floating toolbar, custom tool placement, keyboard shortcut editor, workspace presets, multiple references, split screen | ⬜ |

## 5. Learning system (skill tree, lessons, exercises)
| Feature | Status |
|---|---|
| Glossary, technique explanations inside steps, pressure/shading/colour guides | ✅ |
| Skill tree (10 categories), technique library, guided exercises, practice templates, history, milestones, bookmarks, notes | ⬜ |

## 6. Drawing analysis (no AI)
| Feature | Status |
|---|---|
| Reference vs drawing comparison: side by side, opacity, difference overlay, alignment | ✅ |
| Grayscale comparison | ✅ |
| Histogram, brightness distribution, contrast analysis, symmetry, grid comparison, manual measurements and annotations, colour sampling | ⬜ |

## 7. 3D construction sandbox
⬜ All items (cube, sphere, cylinder, cone, pyramid, combinations, rotation/scale/position, perspective/orthographic camera, vanishing points, lights and shadows, wireframe/solid/silhouette, saved scenes, screenshots).

## 8. Personal library
| Feature | Status |
|---|---|
| Project library, thumbnails, recent, search, automatic local saving, offline access, import/export, backup | ✅ |
| Folders, tags, favourites, covers, notes, version history, before/after | ⬜ |

## 9. Customization
Overlay opacity, grid spacing, paper dimensions, text scale and motion are adjustable ✅. Custom themes/accents, guide colours/thickness, shortcuts, toolbars, templates ⬜.

## 10. Advanced drawing modes
Style presets exist (sketch, realistic, anime, etc.) 🟡. Dedicated portrait / landscape / still life / figure / architecture / manga / technical / symmetry modes ⬜.

## 11. Gesture and timed practice
⬜ All items (30 s – 5 min timers, random reference rotation, pose collections, session history, statistics, pause/resume).

## 12. Export and sharing
Guides, steps, project files, print/PDF guide, images, timelapses ✅. Studio PNG and 2× export ✅. Progress comparison export, print-ready paper templates 🟡.

## 13. Platform
Web: HTML/CSS/JS, responsive, canvas, camera, IndexedDB, offline service worker, PWA ✅; WebGL for the 3D sandbox ⬜.
Desktop: Rust core shared with the web algorithms, native performance, multithreaded processing, native files, Windows/macOS (universal)/Linux x64+ARM, in-app updates ✅; GPU filters ⬜.
Privacy: local-first, no account, no data collection, camera permission explained ✅; optional cloud sync ⬜ (would be opt-in).

## Build order (next)
1. ~~Perspective and proportion guide overlays; line-weight chart~~ (done: Guides).
2. Gesture and timed practice (§11) with pose/reference collections from the user's own pictures.
3. 3D construction sandbox (§7).
4. Skill tree, lessons and exercises (§5).
5. Analysis tools: histogram, symmetry, measurements (§6).
6. Workspace themes (paper, graphite, accent colours), library folders/tags/version history.
7. Desktop ports of the Studio and the new web modules.
