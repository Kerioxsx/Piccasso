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
Shapes are drawn largest first, ellipses from their top clockwise; closed contours start at their top-most point; shading steps demonstrate alternating hatching away from the light. Auto-draw line art: threshold → Zhang–Suen thinning → skeleton tracing (endpoints first, preferring straight continuation) → RDP → greedy nearest-endpoint ordering with reversal. The drawing is fitted (contain) into the chosen screen rectangle, densified to ≤ N px steps and replayed at a fixed speed; a > 25 px difference between the expected and actual cursor position aborts.

## Geometry and comparison
Homography by Gaussian elimination with partial pivoting (8 × 8). Paper detection: largest bright Otsu component, corners at extreme `x+y` / `x−y`; confidence from area and quad fill. Comparison: warp to the paper rectangle, pencil lines = pixels < 200 and 18 darker than the local mean, robust (2–98 %) boxes for size/offset, guide coverage per 3 × 3 cell.

## Styles
Pencil sketch: colour dodge of the gray with a blurred inverse, mixed with 45 % of the original tone, darkened by `1 + sketchContrast/50` (light sketch ×0.4, basic shapes ×0.25). Clean outline: colour Sobel + percentile threshold + dilation by line thickness. Simplified outline: borders between posterized bands. Contour only: subject-mask boundary. Shading guide: flat band tones with darker band borders.
