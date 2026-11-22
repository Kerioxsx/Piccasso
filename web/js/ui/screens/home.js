import { coverOf } from '../../core/history.js';
import { app } from '../../state.js';
import { h, icon, toast, nav, pickFile } from '../dom.js';
import { fileToCanvas, canvasToSource, canvasToRGBA, thumbnail, sourceToDataUrl } from '../imageio.js';
import { createProject, importProject } from '../../core/project.js';

export function renderOnboarding(main) {
  const done = () => { app.settings.onboarded = true; app.saveSettings(); nav('home'); };
  const mode = (ic, title, text) => h('div.card', icon(ic), h('h3', title), h('p.muted', text));
  main.appendChild(h('div.onboard.stack',
    h('h1', 'Welcome to Piccasso'),
    h('p.instruction', 'Pick any picture and Piccasso will help you draw it on real paper, one easy step at a time. No drawing experience needed.'),
    h('div.grid.three',
      mode('steps', 'Guided steps', 'The picture is broken into simple shapes, outlines and shading, with plain-language instructions made for your picture.'),
      mode('lock', 'Locked reference', 'Keep the guide on screen without it moving when you bump the phone. The screen stays awake while you draw.'),
      mode('ar', 'Camera tracing', 'Look at your paper through the camera and see the guide on top of it, so you can trace. Works best on supported phones.')),
    h('p.small.muted', 'Your pictures stay on this device. Nothing is uploaded, and no AI is used.'),
    h('div.row', h('button.btn.primary', { type: 'button', on: { click: done } }, 'Get started'),
      h('button.btn', { type: 'button', on: { click: async () => { done(); await startDemo(); } } }, 'Try the demo first'))));
}

export async function startFromFile(file, { capture = false } = {}) {
  if (!file) return;
  try {
    const canvas = await fileToCanvas(file);
    const source = await canvasToSource(canvas, file.type === 'image/png');
    const base = (file.name || '').replace(/\.[^.]+$/, '');
    const p = createProject({ name: capture || !base || base === 'image' ? `Drawing ${new Date().toLocaleDateString()}` : base, source });
    p.thumb = thumbnail(canvas);
    p.ar.opacity = app.settings.arOpacity;
    await app.setProject(p, { src: canvasToRGBA(canvas) });
    await app.saveProject();
    nav('prepare');
  } catch (e) { toast(e.message, { error: true, ms: 6000 }); }
}

export async function startDemo() {
  try { await app.loadDemo(); nav('workspace'); } catch (e) { toast(e.message, { error: true }); }
}

export async function importProjectFile() {
  const f = await pickFile('.json,application/json');
  if (!f) return;
  try {
    const p = importProject(await f.text());
    const existing = await app.store.get(p.id);
    if (existing) p.id = (await import('../../core/project.js')).uuid();
    if (!p.thumb) {
      const img = new Image(); img.src = sourceToDataUrl(p.source); await img.decode();
      p.thumb = thumbnail(img);
    }
    await app.setProject(p);
    await app.saveProject();
    toast(`Imported “${p.name}”`);
    nav(p.steps.length ? 'workspace' : 'prepare');
  } catch (e) { toast(e.message, { error: true, ms: 7000 }); }
}

export async function openProject(id) {
  try {
    const p = await app.openProject(id);
    nav(p.steps.length ? 'workspace' : 'prepare');
  } catch (e) { toast(e.message, { error: true }); }
}

export function projectCard(p, extra) {
  const pct = p.steps && p.steps.length ? Math.round(((p.progress.completed.length + p.progress.skipped.length) / p.steps.length) * 100) : 0;
  return h('div.project',
    h('img', { src: coverOf(p), alt: '' }),
    h('div.meta',
      h('div.name', p.name),
      h('div.small.muted', p.steps.length ? `Step ${p.progress.currentStep + 1} of ${p.steps.length}` : 'Not started'),
      h('div.progressbar', { role: 'progressbar', 'aria-valuenow': pct, 'aria-valuemin': 0, 'aria-valuemax': 100, 'aria-label': `${p.name} progress` }, h('i', { style: { width: pct + '%' } })),
      extra || null),
    h('button.btn.small', { type: 'button', 'aria-label': `Open ${p.name}`, on: { click: () => openProject(p.id) } }, 'Open'));
}

export async function renderHome(main) {
  const tile = (cls, ic, title, sub, onClick) => h('button.tile' + cls, { type: 'button', on: { click: onClick } }, icon(ic), h('strong', title), h('span', sub));
  const recent = h('div.project-list', h('p.muted', 'Loading…'));
  main.appendChild(h('div',
    h('section.hero',
      h('div',
        h('h1', 'What would you like to draw?'),
        h('p.instruction.muted', 'Choose a picture. Piccasso turns it into simple steps you can follow on paper.'),
        h('div.action-tiles',
          tile('.primary', 'image', 'Import picture', 'From your photos or files', async () => startFromFile(await pickFile('image/*'))),
          tile('', 'camera', 'Take a photo', 'Use your camera', async () => startFromFile(await pickFile('image/*', 'environment'), { capture: true })),
          tile('', 'demo', 'Try the demo', 'A pear and a mug', startDemo),
          tile('', 'file', 'Open project file', '.piccasso.json', importProjectFile))),
      h('div.hero-art', h('img', { src: 'demo/hero.svg', alt: '' }))),
    h('section.stack',
      h('div.row', h('h2', { style: { margin: 0 } }, 'Recent projects'), h('a', { href: '#/library', style: { marginLeft: 'auto' } }, 'All projects')),
      recent),
    h('section.stack', { style: { marginTop: '28px' } },
      h('h2', 'Three ways to draw'),
      h('div.grid.three',
        h('div.card', h('h3', 'Guided'), h('p.muted', 'Step-by-step stages from basic shapes to shading.')),
        h('div.card', h('h3', 'Locked reference'), h('p.muted', 'A steady, touch-locked guide while you draw on paper.')),
        h('div.card', h('h3', 'Camera tracing'), h('p.muted', 'See the guide over your paper through the camera.'))),
      h('p', h('a', { href: '#/learn' }, 'New to drawing? Learn the basic words →')))));
  try {
    const list = (await app.store.list()).slice(0, 6);
    recent.innerHTML = '';
    if (!list.length) recent.appendChild(h('div.empty', 'No projects yet. Import a picture or try the demo to begin.'));
    for (const p of list) recent.appendChild(projectCard(p));
  } catch (e) {
    recent.innerHTML = '';
    recent.appendChild(h('div.notice', 'Saved projects are unavailable: ' + e.message + ' (Private browsing can block local storage.)'));
  }
}

export function renderLearn(main) {
  const g = app.shared.glossary;
  main.appendChild(h('div.stack', { style: { maxWidth: '760px' } },
    h('a.card.skills-link', { href: '#/skills' }, h('strong', 'Skill tree: 30 lessons'), h('span.small.muted', 'From basic shapes to finished portraits, step by step, with practice tools for each lesson.')),
    h('h1', 'Drawing words, explained'),
    h('p.muted', 'You don\'t need to know these to start. Piccasso links to them from each step.'),
    h('dl.glossary', g.terms.flatMap((t) => [h('dt', { id: `term-${t.id}` }, t.term), h('dd', t.text)])),
    h('h2', 'How the steps work'),
    h('ol', ['Mark where the drawing goes on your page.', 'Block in big, simple shapes lightly.', 'Check sizes and positions against each other.',
      'Draw the outside edge, then the big inner shapes.', 'Add smaller features, clean up, then shade from dark to light.'].map((t) => h('li', t))),
    h('h2', 'Good habits for beginners'),
    h('ul', ['Use a regular HB pencil and draw lightly at first.', 'Keep an eraser handy; mistakes are part of the process.',
      'Rest your phone on a stand or lean it on a book so it stays put.', 'Take breaks and step back to look at your drawing from a distance.'].map((t) => h('li', t)))));
}
