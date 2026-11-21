// Gesture and timed practice: timed poses from your own reference collection,
// your current project or a built-in mannequin that strikes random natural
// poses; silhouette and mirror options; pause/resume; history and statistics.
import { app } from '../../state.js';
import { h, segmented, toggle, slider, toast } from '../dom.js';
import { PRESETS, planSession, randomPose, mannequin, mannequinBounds, rng, stats } from '../../core/practice.js';
import { silhouette } from '../../core/filters.js';
import { fitDims, resizeBox, flip } from '../../core/image.js';
import { fileToCanvas, canvasToRGBA, rgbaToCanvas } from '../imageio.js';
import { WakeLock } from '../wakelock.js';

const HISTORY_KEY = 'piccasso.practice.history';
const loadHistory = () => { try { return JSON.parse(localStorage.getItem(HISTORY_KEY)) || []; } catch { return []; } };
const saveHistory = (h_) => { try { localStorage.setItem(HISTORY_KEY, JSON.stringify(h_.slice(-500))); } catch { /* storage off */ } };

// ----- reference collection (IndexedDB, stays on this device) -----
const DB = 'piccasso-practice';
function db() {
  return new Promise((res, rej) => {
    const r = indexedDB.open(DB, 1);
    r.onupgradeneeded = () => r.result.createObjectStore('refs', { keyPath: 'id' });
    r.onsuccess = () => res(r.result); r.onerror = () => rej(r.error);
  });
}
async function tx(mode, fn) { const d = await db(); return new Promise((res, rej) => { const t = d.transaction('refs', mode); const out = fn(t.objectStore('refs')); t.oncomplete = () => res(out && out.result !== undefined ? out.result : out); t.onerror = () => rej(t.error); }); }
const listRefs = () => tx('readonly', (s) => s.getAll());
const addRef = (rec) => tx('readwrite', (s) => s.put(rec));
const delRef = (id) => tx('readwrite', (s) => s.delete(id));

function pickFiles() {
  return new Promise((resolve) => {
    const input = h('input', { type: 'file', accept: 'image/*', multiple: true, style: { display: 'none' } });
    input.addEventListener('change', () => { resolve([...(input.files || [])]); input.remove(); });
    input.addEventListener('cancel', () => { resolve([]); input.remove(); });
    document.body.appendChild(input); input.click();
  });
}

export async function renderPractice(main) {
  const cfg = { source: 'mannequin', preset: 'gesture30', poses: PRESETS.gesture30.poses, seconds: 30, custom: false, show: 'picture', mirror: false, beep: true };
  let refs = [];
  try { refs = await listRefs(); } catch { /* private mode: collection unavailable */ }
  const wake = new WakeLock();
  let session = null; // { plan, i, left, paused, timer, started, drawn }

  const root = h('div.practice');
  main.appendChild(root);

  // ---------- setup ----------
  function renderSetup() {
    const s = stats(loadHistory());
    const grid = h('div.practice-refs');
    const renderRefs = () => grid.replaceChildren(...refs.map((r) => {
      const img = h('img', { alt: r.name, src: URL.createObjectURL(r.blob), loading: 'lazy' });
      return h('div.practice-ref', img, h('button.icon-btn.small', { type: 'button', 'aria-label': `Remove ${r.name}`, on: { click: async () => { await delRef(r.id); refs = refs.filter((x) => x.id !== r.id); renderRefs(); } } }, '×'));
    }));
    renderRefs();
    const addBtn = h('button.btn.small', { type: 'button', on: { click: async () => {
      const files = await pickFiles();
      for (const f of files) {
        try {
          // store a reasonably sized copy
          const c = await fileToCanvas(f);
          const rgba = canvasToRGBA(c), [w, hh] = fitDims(rgba.width, rgba.height, 1600);
          const small = rgbaToCanvas(w === rgba.width ? rgba : resizeBox(rgba, w, hh));
          const blob = await new Promise((r) => small.toBlob(r, 'image/jpeg', 0.88));
          const rec = { id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`, name: f.name, blob, added: Date.now() };
          await addRef(rec); refs.push(rec);
        } catch (e) { toast(`${f.name}: ${e.message}`, { error: true }); }
      }
      renderRefs();
      if (files.length) { cfg.source = 'collection'; renderSetup(); }
    } } }, 'Add pictures');

    const presetChips = h('div.chips', ...Object.entries(PRESETS).map(([k, p]) => h('button.chip', { type: 'button', 'aria-pressed': String(!cfg.custom && cfg.preset === k),
      on: { click: () => { cfg.preset = k; cfg.custom = false; cfg.poses = p.poses; renderSetup(); } } }, p.label)),
      h('button.chip', { type: 'button', 'aria-pressed': String(cfg.custom), on: { click: () => { cfg.custom = true; renderSetup(); } } }, 'Custom'));

    root.replaceChildren(h('div.stack', { style: { maxWidth: '820px' } },
      h('h1', 'Timed practice'),
      h('p.muted', 'Quick timed poses train your eye to see the whole movement first. Draw the big line of action, then the shapes; stop when the time runs out.'),
      h('section.card.stack', h('h2', 'What to draw'),
        segmented('Reference source', [['mannequin', 'Mannequin poses'], ['collection', `My pictures (${refs.length})`], ...(app.src ? [['project', 'Current project']] : [])], cfg.source, (v) => { cfg.source = v; renderSetup(); }),
        cfg.source === 'mannequin' ? h('p.small.muted', 'A simple jointed figure in a new natural pose each time. Good for gesture and proportion.') : null,
        cfg.source === 'collection' ? h('div.stack', h('p.small.muted', 'Your own pose and figure references. They stay on this device.'), h('div.row', addBtn), grid) : null,
        segmented('Show as', [['picture', 'Picture'], ['silhouette', 'Silhouette only']], cfg.show, (v) => { cfg.show = v; }),
        toggle('Mirror the reference', cfg.mirror, (v) => { cfg.mirror = v; }, 'Draw it flipped: a classic way to stop copying details and see the shape.')),
      h('section.card.stack', h('h2', 'Timing'),
        presetChips,
        cfg.custom ? h('div.grid', { style: { gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))' } },
          slider({ label: 'Poses', min: 1, max: 40, value: cfg.poses, onInput: (v) => { cfg.poses = v; } }),
          slider({ label: 'Seconds per pose', min: 10, max: 900, step: 10, value: cfg.seconds, format: (v) => (v < 60 ? `${v} s` : `${Math.floor(v / 60)} min${v % 60 ? ` ${v % 60} s` : ''}`), onInput: (v) => { cfg.seconds = v; } })) : null,
        toggle('Soft beep when the pose changes', cfg.beep, (v) => { cfg.beep = v; })),
      h('button.btn.primary', { type: 'button', style: { alignSelf: 'start', minWidth: '200px' }, on: { click: start } }, 'Start practice'),
      h('section.card.stack', h('h2', 'Your practice'),
        h('div.practice-stats',
          ...[[s.minutes, 'minutes in total'], [s.weekMinutes, 'minutes this week'], [s.poses, 'poses drawn'], [s.streak, s.streak === 1 ? 'day streak' : 'days streak']].map(([n, l]) => h('div', h('strong', String(n)), h('span.small.muted', l)))),
        historyList())));
  }

  function historyList() {
    const hist = loadHistory().slice(-8).reverse();
    if (!hist.length) return h('p.small.muted', 'No sessions yet.');
    return h('ul.practice-history', ...hist.map((x) => h('li', h('span', new Date(x.at).toLocaleString()), h('span.small.muted', `${x.poses} poses · ${Math.round(x.seconds / 60)} min · ${x.label}`))));
  }

  // ---------- session ----------
  const stage = h('canvas.practice-canvas', { 'aria-label': 'Reference' });
  const clock = h('div.practice-clock', { role: 'timer', 'aria-live': 'off' });
  const counter = h('div.small');
  const pauseBtn = h('button.btn', { type: 'button', on: { click: () => togglePause() } }, 'Pause');

  async function start() {
    const seconds = cfg.custom ? [cfg.seconds] : PRESETS[cfg.preset].seconds;
    const poses = cfg.custom ? cfg.poses : PRESETS[cfg.preset].poses;
    let ids;
    if (cfg.source === 'collection') { if (!refs.length) { toast('Add some pictures first, or choose mannequin poses.'); return; } ids = refs.map((r) => r.id); }
    else if (cfg.source === 'project') ids = ['project'];
    else ids = Array.from({ length: poses }, (_, i) => `pose-${i}`);
    const seed = Date.now() & 0x7fffffff;
    session = { plan: planSession({ refs: ids, poses, seconds, seed }), i: -1, left: 0, paused: false, timer: 0, drawn: 0, rand: rng(seed), label: cfg.custom ? 'custom' : PRESETS[cfg.preset].label };
    root.replaceChildren(h('div.practice-session', stage,
      h('div.practice-bar', clock, counter,
        h('div.row', pauseBtn,
          h('button.btn', { type: 'button', on: { click: () => next() } }, 'Next pose'),
          h('button.btn', { type: 'button', on: { click: () => finish() } }, 'End')))));
    document.addEventListener('keydown', onKey);
    wake.request().catch(() => {});
    next();
    session.timer = setInterval(tick, 250);
  }

  function onKey(e) {
    if (!session) return;
    if (e.key === ' ') { e.preventDefault(); togglePause(); }
    else if (e.key === 'ArrowRight') next();
    else if (e.key === 'Escape') finish();
  }

  function togglePause() { session.paused = !session.paused; pauseBtn.textContent = session.paused ? 'Resume' : 'Pause'; tick(); }

  let last = 0;
  function tick() {
    if (!session) return;
    const now = performance.now();
    if (!session.paused && last) { const d = (now - last) / 1000; session.left -= d; session.drawn += d; }
    last = now;
    if (session.left <= 0 && !session.paused) { next(); return; }
    const l = Math.max(0, Math.ceil(session.left));
    clock.textContent = session.paused ? 'Paused' : `${Math.floor(l / 60)}:${String(l % 60).padStart(2, '0')}`;
    clock.classList.toggle('ending', l <= 5 && !session.paused);
  }

  function beep() {
    if (!cfg.beep) return;
    try { const a = new AudioContext(), o = a.createOscillator(), g = a.createGain(); o.frequency.value = 660; g.gain.value = 0.05; o.connect(g).connect(a.destination); o.start(); o.stop(a.currentTime + 0.15); o.onended = () => a.close(); } catch { /* no audio */ }
  }

  async function next() {
    session.i++;
    if (session.i >= session.plan.length) { finish(); return; }
    const item = session.plan[session.i];
    session.left = item.seconds; last = performance.now();
    counter.textContent = `Pose ${session.i + 1} of ${session.plan.length} · ${item.seconds < 60 ? `${item.seconds} s` : `${item.seconds / 60} min`}`;
    if (session.i > 0) beep();
    await drawRef(item.ref);
    tick();
  }

  async function drawRef(ref) {
    let rgba;
    if (cfg.source === 'collection') {
      const r = refs.find((x) => x.id === ref);
      const bmp = await createImageBitmap(r.blob);
      const c = h('canvas', { width: bmp.width, height: bmp.height }); c.getContext('2d').drawImage(bmp, 0, 0);
      rgba = canvasToRGBA(c);
    } else if (cfg.source === 'project') rgba = app.src;
    if (rgba) {
      if (cfg.show === 'silhouette') rgba = silhouette(rgba);
      if (cfg.mirror) rgba = flip(rgba, true, false);
      rgbaToCanvas(rgba, stage);
      return;
    }
    // mannequin
    const W = 900, H = 1100;
    stage.width = W; stage.height = H;
    const g = stage.getContext('2d');
    g.fillStyle = '#fbfaf6'; g.fillRect(0, 0, W, H);
    const m0 = mannequin(randomPose(session.rand), [0, 0], 800);
    const b = mannequinBounds(m0), k = Math.min((W * 0.85) / b.w, (H * 0.88) / b.h, 1.2);
    const ox = W / 2 - (b.x + b.w / 2) * k, oy = H / 2 - (b.y + b.h / 2) * k;
    const P = (p) => [(cfg.mirror ? -1 : 1) * (p[0] * k) + (cfg.mirror ? W - ox : ox), p[1] * k + oy];
    const sil = cfg.show === 'silhouette';
    // the side turned away is darker, so the pose reads in depth
    const tone = (side) => (sil ? '#222' : side === 0 ? '#c4ad8c' : (side > 0) !== cfg.mirror ? '#d9c8ad' : '#a88f6c');
    const ground = Math.max(P(m0.joints.footL)[1], P(m0.joints.footR)[1]) + 0.25 * m0.unit * k;
    if (!sil) { g.strokeStyle = 'rgba(0,0,0,.18)'; g.lineWidth = 2; g.beginPath(); g.moveTo(W * 0.1, ground); g.lineTo(W * 0.9, ground); g.stroke(); }
    g.lineCap = 'round'; g.lineJoin = 'round';
    const limb = (bone) => { g.strokeStyle = tone(bone.side); g.lineWidth = bone.w * k; g.beginPath(); g.moveTo(...P(bone.a)); g.lineTo(...P(bone.b)); g.stroke(); };
    // far limbs first, then torso and head, then near limbs
    const far = (b0) => (b0.side > 0) !== cfg.mirror && b0.side !== 0;
    m0.bones.filter(far).forEach(limb);
    g.fillStyle = tone(0); g.strokeStyle = tone(0); g.lineWidth = 0.18 * m0.unit * k;
    g.beginPath(); m0.torso.forEach((p, i) => (i ? g.lineTo(...P(p)) : g.moveTo(...P(p)))); g.closePath(); g.fill(); g.stroke();
    m0.bones.filter((b0) => b0.side === 0).forEach(limb);
    g.fillStyle = tone(0); g.beginPath(); g.arc(...P(m0.joints.head), m0.headR * k * 0.85, 0, Math.PI * 2); g.fill();
    m0.bones.filter((b0) => !far(b0) && b0.side !== 0).forEach(limb);
    if (!sil) {
      g.fillStyle = 'rgba(80,60,40,.55)';
      for (const key of ['shoulderL', 'shoulderR', 'elbowL', 'elbowR', 'hipL', 'hipR', 'kneeL', 'kneeR']) { g.beginPath(); g.arc(...P(m0.joints[key]), 0.07 * m0.unit * k, 0, Math.PI * 2); g.fill(); }
      // line of action: head → chest → hips → the foot carrying the weight
      const foot = m0.joints.footL[1] > m0.joints.footR[1] ? m0.joints.footL : m0.joints.footR;
      const pts = [m0.joints.head, m0.joints.chest, m0.joints.hip, foot].map(P);
      g.strokeStyle = 'rgba(42,122,108,.85)'; g.lineWidth = 3; g.setLineDash([10, 8]);
      g.beginPath(); g.moveTo(...pts[0]); g.bezierCurveTo(...pts[1], ...pts[2], ...pts[3]); g.stroke(); g.setLineDash([]);
    }
  }

  function finish() {
    if (!session) return;
    clearInterval(session.timer);
    document.removeEventListener('keydown', onKey);
    wake.release().catch(() => {});
    const done = Math.max(0, Math.min(session.plan.length, session.i + (session.left <= 0 ? 1 : 0)));
    if (session.drawn > 5) {
      const hist = loadHistory();
      hist.push({ at: Date.now(), poses: Math.max(1, done), seconds: Math.round(session.drawn), label: session.label });
      saveHistory(hist);
    }
    const drawn = Math.round(session.drawn);
    session = null; last = 0;
    renderSetup();
    if (drawn > 5) toast(`Session saved: ${Math.round(drawn / 60) || '<1'} min of drawing.`);
  }

  renderSetup();
  return () => { if (session) { clearInterval(session.timer); document.removeEventListener('keydown', onKey); wake.release().catch(() => {}); session = null; } };
}
