# Project format v1

Projects are JSON files with the extension `.piccasso.json`, readable and writable by both apps. Schema: [`schema/piccasso-project.v1.schema.json`](../schema/piccasso-project.v1.schema.json). Example: [`samples/demo-project.piccasso.json`](../samples/demo-project.piccasso.json).

Rules:
- `format` is `"piccasso-project"`, `version` is an integer. Readers reject newer versions with a clear message and fill defaults for missing optional sections.
- `source.data` is the base64 image (long side ≤ 2048 px). Processed images are **not** stored in the file; they are regenerated from `source` + `prep` (deterministically, identically in both apps).
- `analysis` and `steps` are stored so a project opens instantly and so steps written by older versions' optional helper (marked `source: "ai"`, with `localInstruction` kept) still open; current versions show the built-in `localInstruction` for them.
- `progress` holds the current step, completed and skipped step ids and time spent; `view` and `ar` hold the locked-view and camera-overlay settings.
- **Unknown fields must be preserved** when re-saving. The Rust app keeps them in a flattened map (tested), so the web app's `thumb` field (and the legacy `ai` field) survive a round trip through the desktop app.

Versioning: additive, optional fields keep version 1 (e.g. `prep.medium`, `prep.paper.widthMm/heightMm/gridCm`, `analysis.technique`, `steps[].motion` were added this way). A breaking change increments `version`; readers then migrate or refuse.
