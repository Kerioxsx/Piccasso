# Algorithms (local-v1)

All coordinates in an analysis are normalized: points are `(x / width, y / height)`; ellipse radii `rx`, `ry` are fractions of the image **width**. Rounding of stored numbers uses symmetric `r4(v) = sign(v) · floor(|v| · 10⁴ + 0.5) / 10⁴`. Wherever JavaScript uses `Math.round`, Rust uses `floor(x + 0.5)`.

## Preparation
rotate (90° steps, clockwise) → flip → crop (normalized rect relative to the rotated image) → area-average resize to fit `maxDim` (never upscale) → composite on white and apply `v' = round((v − 128)·(100 + contrast)/100 + 128 + 2.55·brightness)`.

## Primitives
- Gray: `(77R + 150G + 29B + 128) >> 8`.
- Blur: separable box blur with edge clamping and rounded integer mean; `blur2` = two passes.
- Sobel: L1 magnitude `>> 2`, clamped to 255; colour Sobel = max over blurred R, G, B.
- Percentile threshold: smallest `t ≥ minEdge` with at most `edgeFraction` of pixels `≥ t`.
- Otsu threshold on the histogram.
- Morphology: square dilation/erosion; subject masks use close(1) then open(2).
- Components: 4-connected flood fill in raster order (stack, neighbours left/right/up/down).
- Boundary: Moore-neighbour trace from the component's first raster pixel, clockwise, search starting 90° counter-clockwise of the last move; stops on return to start.
- Simplification: iterative Ramer–Douglas–Peucker (`eps = dpEpsilon · max(W, H)`).

## Subject
Border colours are found with k-means (k = 3, 8 iterations, initialised at 10/50/90 % luminance quantiles) over the outer 2-px band of the blurred image. Each pixel's distance to the nearest border colour (mean absolute RGB difference) is thresholded with `max(20, otsu)`. Components touching ≥ 3 borders and covering > 50 % are background. If the class separation is < 18 or nothing remains, the image is "low contrast" and the whole frame becomes the subject.

## Plan contents
- Objects: largest components (`maxObjects`); silhouettes: traced + simplified boundaries of up to 3 active objects.
- Value bands: posterize `blur2(gray, 2·blurRadius + 1)` into `valueLevels` bands; darkest/lightest bands with ≥ 1 % of the subject.
- Light direction: vector from the darkness-weighted to the brightness-weighted centroid; magnitude < 0.04 of the subject size means "even"; otherwise a 3 × 3 direction code.
- Internal forms: components of the darkest and lightest bands inside the subject (not > 90 % of it), largest first.
- Construction shapes: second-moment ellipse fit (`r = 2√λ`); fill ratio > 0.85 → box; axis ratio > 0.85 → circle.
- Detail: colour-Sobel edges per 3 × 3 cell of the subject box.
- Technique: per value band, a label anchor (band pixel nearest the band centroid); palette by k-means (k = 5, init at 10/30/50/70/90 % luminance quantiles, L1 RGB, 8 iterations), clusters ≥ 3 %. Colour names use a "redmean" weighted RGB distance plus a chroma term. Pressure entries come from `shared/pressure.json` by rank among the bands present.

## Steps
Kinds come from the difficulty level, then: `forms` needs ≥ 2 internal forms; `features` is dropped without internal forms and low detail; `pressure` needs ≥ 2 bands; `color` is added after `values` when the medium is colour; contour-only removes shading steps. Text is filled from templates with computed values (paper layout, position words, aspect phrase, shape list and size comparison, turn count of the outline, pressure list, palette list, shading method, grid squares).

## Motion paths and auto-draw strokes
Shapes are drawn largest first, ellipses from their top clockwise; closed contours start at their top-most point; shading steps demonstrate alternating hatching away from the light. Auto-draw line art: threshold → Zhang–Suen thinning → skeleton tracing (endpoints first, preferring straight continuation) → RDP → greedy nearest-endpoint ordering with reversal. The drawing is fitted (contain) into the chosen screen rectangle; a > 25 px difference between the expected and actual cursor position aborts.

### Smooth pen timing (`penpath.rs`)
Drawing programs read the mouse `hz` times a second and connect the readings with straight lines. For each vertex of a stroke Piccasso works out the speed that keeps those chords within `tol` (0.5 px) of the path:
- a curve of radius R allows `v ≤ hz·√(8·R·tol)` (chord sagitta s²/8R ≤ tol with s = v/hz), with R ≈ segment length / turn angle;
- a vertex where even the shortest neighbouring segment would miss the corner by more than `tol` (`min(len)·sin(turn/2) > tol`) is a real corner: the pen holds there for 1.3 reading periods, so a reading lands exactly on it and both straight runs stay exact at full speed.
Speeds are limited by an acceleration ramp in both directions, integrated along each segment (accelerate, cruise, brake), and the mouse position is emitted every millisecond. A test samples the result like a 60 Hz program at several phases: a circle, a zigzag and a serpentine fill stay within 1 px.

### Full-resolution line capture (`lines.rs`)
Lines are found on the picture at up to 3× the drawing size, where a 1 px ink line is still solid: black-hat filter (closing − original ≥ 48 levels, window ≈ the fill brush) → thinning → skeleton tracing → RDP (0.6) → two rounds of Chaikin smoothing. Each line keeps its darkness, the opacity needed over the shading beneath it, its width (area / centre-line length) and its colour. The lines are lifted out of the picture (replaced by the closing) before the shading is planned, then drawn on top as single smooth strokes, grouped by opacity (0.3/0.55/0.8/1.0) and width class (multiples of the line brush). Region outlines are smoothed with Chaikin as well.

### Colour replica
A deterministic palette: one centre per clearly present hue (12 hue bins of pixels with chroma ≥ 50), the rest at brightness quantiles of the low-chroma pixels, refined by k-means with a distance that weights chroma 1.5× luma, so a small green eye never merges into a grey of the same brightness. Colours are painted opaque, lightest first; each colour's mask reaches one brush radius under the darker colours painted after it, so neighbours meet with no white gaps. Ink lines take the nearest palette colour.

### Reading the target program (`tooldetect.rs`)
- **Colour picker detection**: 8 px blocks that change colour smoothly (range ≥ 6 levels, no neighbour step > 40) are joined into widgets; a widget must fill ≥ 35 % of its box and its gradient pixels must span ≥ 120 levels (toolbars, buttons and icons don't).
- **Choosing a colour**: an area with ≥ 5 hues present is a hue picker (wheel/hue strip) and is matched on hue and saturation; others (saturation/brightness square, value slider) on the full colour. Piccasso clicks the best pixel of each area in order, re-reading the screen after every click (the other pickers change), up to 4 rounds until the optional swatch matches. The panel colour in the box corners is never clicked.
- **Brush size without numbers**: black at 100 %, then for slider positions 0, 4, 10, 18, 30, 45, 65, 85, 100 % a short line in the test spot; its thickness is measured on screen. The widths form a monotone curve; a wanted size is found by interpolating in log width. Tested on a slider with size = 1 + 59·t² (no labels).

## Geometry and comparison
Homography by Gaussian elimination with partial pivoting (8 × 8). Paper detection: largest bright Otsu component, corners at extreme `x+y` / `x−y`; confidence from area and quad fill. Comparison: warp to the paper rectangle, pencil lines = pixels < 200 and 18 darker than the local mean, robust (2–98 %) boxes for size/offset, guide coverage per 3 × 3 cell.

## Styles
Pencil sketch: colour dodge of the gray with a blurred inverse, mixed with 45 % of the original tone, darkened by `1 + sketchContrast/50` (light sketch ×0.4, basic shapes ×0.25). Clean outline: colour Sobel + percentile threshold + dilation by line thickness. Simplified outline: borders between posterized bands. Contour only: subject-mask boundary. Shading guide: flat band tones with darker band borders.
