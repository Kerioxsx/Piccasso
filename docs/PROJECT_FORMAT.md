# Project format v1

Projects are JSON files with the extension `.drawstep.json`, readable and writable by both apps. Schema: [`schema/drawstep-project.v1.schema.json`](../schema/drawstep-project.v1.schema.json). Example: [`samples/demo-project.drawstep.json`](../samples/demo-project.drawstep.json).

Rules:
- `format` is `"drawstep-project"`, `version` is an integer. Readers reject newer versions with a clear message and fill defaults for missing optional sections.
- `source.data` is the base64 image (long side ≤ 2048 px). Processed images are **not** stored in the file; they are regenerated from `source` + `prep` (deterministically, identically in both apps).
- `analysis` and `steps` are stored so a project opens instantly and so AI-written step text (marked `source: "ai"`, with `localInstruction` kept) survives.
- `progress` holds the current step, completed and skipped step ids and time spent; `view` and `ar` hold the locked-view and camera-overlay settings.
- **Unknown fields must be preserved** when re-saving. The Rust app keeps them in a flattened map (tested), so the web app's `thumb` and `ai` fields survive a round trip through the desktop app.

Versioning: additive, optional fields keep version 1 (e.g. `prep.medium`, `prep.paper.widthMm/heightMm/gridCm`, `analysis.technique`, `steps[].motion` were added this way). A breaking change increments `version`; readers then migrate or refuse.
