// Small DOM helpers (no framework).

/** h('div.card#id', {attrs, on:{click}}, ...children) */
export function h(tag, attrs, ...children) {
  const m = /^([a-z0-9-]+)?((?:[.#][\w-]+)*)$/i.exec(tag);
  const el = document.createElement(m[1] || 'div');
  for (const part of (m[2] || '').match(/[.#][\w-]+/g) || []) {
    if (part[0] === '.') el.classList.add(part.slice(1)); else el.id = part.slice(1);
  }
  if (attrs && (typeof attrs !== 'object' || attrs instanceof Node || Array.isArray(attrs))) { children.unshift(attrs); attrs = null; }
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v == null || v === false) continue;
    if (k === 'on') for (const [ev, fn] of Object.entries(v)) el.addEventListener(ev, fn);
    else if (k === 'style' && typeof v === 'object') Object.assign(el.style, v);
    else if (k === 'html') el.innerHTML = v;
    else if (k in el && typeof v !== 'string' && k !== 'list') el[k] = v;
    else el.setAttribute(k, v === true ? '' : v);
  }
  append(el, children);
  return el;
}
function append(el, children) {
  for (const c of children.flat(Infinity)) {
    if (c == null || c === false) continue;
    el.appendChild(c instanceof Node ? c : document.createTextNode(String(c)));
  }
}

const ICONS = {
  image: '<rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="9" cy="10" r="2"/><path d="M21 17l-5-5-9 8"/>',
  camera: '<path d="M4 8h3l2-3h6l2 3h3v11H4z"/><circle cx="12" cy="13" r="3.5"/>',
  demo: '<path d="M4 20c4-1 6-5 8-9s5-7 8-7"/><circle cx="5" cy="19" r="1.5"/>',
  file: '<path d="M6 3h8l4 4v14H6z"/><path d="M14 3v4h4"/>',
  book: '<path d="M4 5c3-1 6-1 8 1 2-2 5-2 8-1v14c-3-1-6-1-8 1-2-2-5-2-8-1z"/><path d="M12 6v14"/>',
  steps: '<path d="M4 18h5v-4h5v-4h5V6"/>',
  lock: '<rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V8a4 4 0 0 1 8 0v3"/>',
  unlock: '<rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V8a4 4 0 0 1 7.5-2"/>',
  ar: '<path d="M3 8V5h3M18 5h3v3M21 16v3h-3M6 19H3v-3"/><path d="M8 9l4-2 4 2v6l-4 2-4-2z"/>',
  prev: '<path d="M15 18l-6-6 6-6"/>', next: '<path d="M9 18l6-6-6-6"/>',
  speak: '<path d="M4 10v4h4l5 4V6L8 10z"/><path d="M16 9a4 4 0 0 1 0 6"/>',
  eye: '<path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>',
  eyeoff: '<path d="M3 3l18 18"/><path d="M10.6 5.1A10 10 0 0 1 12 5c6 0 10 7 10 7a17 17 0 0 1-3.2 3.9M6.6 6.6C3.8 8.4 2 12 2 12s4 7 10 7c1.6 0 3-.4 4.3-1"/>',
  skip: '<path d="M5 6l7 6-7 6zM13 6l7 6-7 6z"/>', restart: '<path d="M4 4v6h6"/><path d="M5 15a8 8 0 1 0 2-9L4 10"/>',
  compare: '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M12 4v16"/>',
  photo: '<path d="M4 8h3l2-3h6l2 3h3v11H4z"/><path d="M9 13l2 2 4-4"/>',
  export: '<path d="M12 3v12M7 8l5-5 5 5"/><path d="M5 15v5h14v-5"/>',
  zoomin: '<circle cx="11" cy="11" r="7"/><path d="M21 21l-5-5M11 8v6M8 11h6"/>',
  zoomout: '<circle cx="11" cy="11" r="7"/><path d="M21 21l-5-5M8 11h6"/>',
  fit: '<path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"/>',
  flip: '<path d="M12 3v18"/><path d="M8 7L3 12l5 5zM16 7l5 5-5 5z"/>',
  rotl: '<path d="M4 4v6h6"/><path d="M5 15a8 8 0 1 0 2-9L4 10"/>',
  rotr: '<path d="M20 4v6h-6"/><path d="M19 15a8 8 0 1 1-2-9l3 4"/>',
  close: '<path d="M6 6l12 12M18 6L6 18"/>', check: '<path d="M5 12l5 5 9-10"/>',
  trash: '<path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13"/>', copy: '<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V4H4v12h4"/>',
  edit: '<path d="M4 20h4L20 8l-4-4L4 16z"/>', timer: '<circle cx="12" cy="13" r="8"/><path d="M12 9v4l3 2M9 2h6"/>',
  target: '<circle cx="12" cy="12" r="8"/><circle cx="12" cy="12" r="3"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3"/>',
  sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M2 12h2M20 12h2M5 5l1.5 1.5M17.5 17.5L19 19M5 19l1.5-1.5M17.5 6.5L19 5"/>',
  help: '<circle cx="12" cy="12" r="9"/><path d="M9.5 9a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6V14M12 17h.01"/>',
};
export function icon(name) {
  const s = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  s.setAttribute('viewBox', '0 0 24 24'); s.setAttribute('aria-hidden', 'true');
  s.innerHTML = ICONS[name] || '';
  return s;
}

export function toast(msg, { error = false, ms = 3500 } = {}) {
  const region = document.getElementById('toast-region');
  const t = h('div.toast' + (error ? '.error' : ''), { role: error ? 'alert' : 'status' }, msg);
  region.appendChild(t);
  setTimeout(() => t.remove(), ms);
}

/** Announce to screen readers (assertive). */
export function announce(msg) {
  const el = document.getElementById('sr-announcer');
  el.textContent = '';
  setTimeout(() => { el.textContent = msg; }, 30);
}

/** Modal dialog. buttons: [{label, value, primary, danger}] → resolves with value (or null on cancel). */
export function dialog({ title, body, buttons = [{ label: 'OK', value: true, primary: true }], dismissable = true }) {
  return new Promise((resolve) => {
    const d = h('dialog', { 'aria-labelledby': 'dlg-title' },
      h('h2#dlg-title', title),
      typeof body === 'string' ? h('p', body) : body,
      h('div.dialog-actions', buttons.map((b) => h('button.btn' + (b.primary ? '.primary' : '') + (b.danger ? '.danger' : ''), {
        type: 'button', on: { click: () => { d.close(); resolve(typeof b.value === 'function' ? b.value() : b.value); } },
      }, b.label))));
    d.addEventListener('cancel', (e) => { if (!dismissable) e.preventDefault(); else resolve(null); });
    d.addEventListener('close', () => setTimeout(() => d.remove(), 0));
    document.body.appendChild(d);
    d.showModal();
  });
}

export async function confirmDialog(title, body, okLabel = 'OK', danger = false) {
  return (await dialog({ title, body, buttons: [{ label: 'Cancel', value: false }, { label: okLabel, value: true, primary: !danger, danger }] })) === true;
}

export async function promptDialog(title, value = '', label = 'Name') {
  const input = h('input', { type: 'text', value, 'aria-label': label, style: { width: '100%' } });
  setTimeout(() => { input.focus(); input.select(); }, 50);
  const ok = await dialog({ title, body: input, buttons: [{ label: 'Cancel', value: false }, { label: 'Save', value: true, primary: true }] });
  return ok ? input.value : null;
}

export function slider({ label, min, max, step = 1, value, format = (v) => v, onInput }) {
  const out = h('output', format(value));
  const input = h('input', { type: 'range', min, max, step, value, 'aria-label': label,
    on: { input: () => { out.textContent = format(+input.value); onInput(+input.value); } } });
  return h('label.field', h('span.field-row', h('span', label), out), input);
}

export function segmented(label, options, value, onChange) {
  const wrap = h('div.segmented', { role: 'group', 'aria-label': label });
  const render = (v) => wrap.querySelectorAll('button').forEach((b) => b.setAttribute('aria-pressed', String(b.dataset.v === String(v))));
  for (const [v, text] of options) {
    wrap.appendChild(h('button', { type: 'button', 'data-v': v, on: { click: () => { render(v); onChange(v); } } }, text));
  }
  render(value);
  return h('fieldset', h('legend', label), wrap);
}

export function toggle(label, checked, onChange, hint) {
  const input = h('input', { type: 'checkbox', role: 'switch', checked, on: { change: () => onChange(input.checked) } });
  return h('label.switch', h('span', h('span', label), hint ? h('div.small.muted', hint) : null), input);
}

export function download(blob, filename) {
  const url = URL.createObjectURL(blob);
  const a = h('a', { href: url, download: filename });
  document.body.appendChild(a); a.click(); a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 5000);
}

export function pickFile(accept, capture) {
  return new Promise((resolve) => {
    const input = h('input', { type: 'file', accept, style: { display: 'none' } });
    if (capture) input.setAttribute('capture', capture);
    input.addEventListener('change', () => { resolve(input.files && input.files[0] || null); input.remove(); });
    input.addEventListener('cancel', () => { resolve(null); input.remove(); });
    document.body.appendChild(input);
    input.click();
  });
}

export function fmtTime(sec) {
  const s = Math.floor(sec % 60), m = Math.floor(sec / 60) % 60, hh = Math.floor(sec / 3600);
  return (hh ? hh + ':' + String(m).padStart(2, '0') : m) + ':' + String(s).padStart(2, '0');
}

/** Navigate to a route without importing the router (avoids import cycles). */
export const nav = (route) => window.dispatchEvent(new CustomEvent('drawstep:navigate', { detail: route }));
