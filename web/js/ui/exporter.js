// Export: reference PNG, printable step sheet, portable project file.
import { download } from './dom.js';
import { composeGuide } from './canvas.js';
import { exportProject } from '../core/project.js';

const safeName = (s) => (s || 'drawing').replace(/[^\w\- ]+/g, '').trim().replace(/\s+/g, '-').slice(0, 60) || 'drawing';

export async function exportPNG(canvas, name) {
  const blob = await new Promise((r) => canvas.toBlob(r, 'image/png'));
  download(blob, `${safeName(name)}.png`);
}

export function exportProjectFile(project) {
  download(new Blob([exportProject(project)], { type: 'application/json' }), `${safeName(project.name)}.drawstep.json`);
}

const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));

/** Builds a printable HTML step sheet and opens it (falls back to download if pop-ups are blocked). */
export function exportStepSheet(project, baseFor) {
  const items = project.steps.map((s, i) => {
    const c = composeGuide(baseFor(s.base), project.analysis, s.layers, { width: 520 });
    return `<section><img src="${c.toDataURL('image/jpeg', 0.85)}" alt="Guide for step ${i + 1}"><div><h2>${i + 1}. ${esc(s.title)}</h2><p>${esc(s.instruction)}</p><p class="tip"><b>Tip:</b> ${esc(s.tip)}</p></div></section>`;
  }).join('\n');
  const html = `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>${esc(project.name)} – DrawStep step sheet</title>
<style>body{font:15px/1.5 system-ui,sans-serif;color:#222;max-width:900px;margin:24px auto;padding:0 16px}h1{font-family:Georgia,serif}section{display:grid;grid-template-columns:260px 1fr;gap:16px;align-items:start;padding:14px 0;border-top:1px solid #ddd;break-inside:avoid}img{width:100%;border:1px solid #ccc;border-radius:8px}h2{font-size:1.05rem;margin:0 0 6px}.tip{color:#555}button{font:inherit;padding:10px 16px;margin-bottom:12px}@media print{button{display:none}}@media(max-width:600px){section{grid-template-columns:1fr}}</style></head>
<body><button onclick="print()">Print / Save as PDF</button><h1>${esc(project.name)}</h1><p>${project.steps.length} steps · difficulty: ${esc(project.prep.difficulty.replace('_', ' '))} · made with DrawStep</p>${items}</body></html>`;
  const blob = new Blob([html], { type: 'text/html' });
  const url = URL.createObjectURL(blob);
  const w = window.open(url, '_blank');
  if (!w) download(blob, `${safeName(project.name)}-steps.html`);
  setTimeout(() => URL.revokeObjectURL(url), 60000);
}
