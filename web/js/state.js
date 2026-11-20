// Application state shared by all screens.
import { ProjectStore, IDBBackend, MemoryBackend } from './core/store.js';
import { importProject } from './core/project.js';
import { sourceToRGBA, rgbaToCanvas } from './ui/imageio.js';

const SETTINGS_KEY = 'drawstep.settings.v1';
export const DEFAULT_SETTINGS = {
  theme: 'system', textScale: 1, motion: 'system', autoRead: false,
  holdToUnlock: true, holdMs: 1200, keepAwake: true, lockTimer: true,
  arOpacity: 0.5, arDefaultLayer: 'outline',
  aiEnabled: false, aiModel: 'claude-opus-5-5', rememberKey: false,
  onboarded: false, lang: 'en',
};

function safeGet(storage, key) { try { return storage.getItem(key); } catch { return null; } }
function safeSet(storage, key, v) { try { storage.setItem(key, v); return true; } catch { return false; } }
function safeDel(storage, key) { try { storage.removeItem(key); } catch { /* ignore */ } }

export function loadSettings() {
  let s = {};
  try { s = JSON.parse(safeGet(localStorage, SETTINGS_KEY) || '{}'); } catch { s = {}; }
  return { ...DEFAULT_SETTINGS, ...s };
}

/** Stable hash of the prep fields that affect renders. */
export function prepHash(prep) {
  const s = JSON.stringify([prep.crop, prep.rotation, prep.flipH, prep.flipV, prep.brightness, prep.contrast, prep.style, prep.difficulty,
    prep.lineThickness, prep.sketchContrast, prep.simplifyBackground, prep.excludedObjects, prep.isolateObject]);
  let h = 0x811c9dc5;
  for (let i = 0; i < s.length; i++) { h ^= s.charCodeAt(i); h = Math.imul(h, 0x01000193) >>> 0; }
  return h.toString(16);
}

/** IndexedDB cache of processed renders ("processed versions") as PNG blobs. */
class RenderCache {
  constructor() { this.db = null; }
  _open() {
    if (!('indexedDB' in globalThis)) return Promise.resolve(null);
    if (!this.db) this.db = new Promise((res) => {
      const r = indexedDB.open('drawstep-renders', 1);
      r.onupgradeneeded = () => r.result.createObjectStore('renders', { keyPath: 'id' });
      r.onsuccess = () => res(r.result);
      r.onerror = () => res(null);
    });
    return this.db;
  }
  async get(id, hash) {
    const db = await this._open();
    if (!db) return null;
    const rec = await new Promise((res) => {
      const q = db.transaction('renders').objectStore('renders').get(id);
      q.onsuccess = () => res(q.result); q.onerror = () => res(null);
    });
    if (!rec || rec.hash !== hash) return null;
    const out = {};
    for (const [k, blob] of Object.entries(rec.images)) {
      const bmp = await createImageBitmap(blob);
      const c = document.createElement('canvas'); c.width = bmp.width; c.height = bmp.height;
      c.getContext('2d').drawImage(bmp, 0, 0); out[k] = c;
    }
    return out;
  }
  async put(id, hash, canvases) {
    const db = await this._open();
    if (!db) return;
    const images = {};
    for (const [k, c] of Object.entries(canvases)) images[k] = await new Promise((r) => c.toBlob(r, 'image/png'));
    await new Promise((res) => {
      const tx = db.transaction('renders', 'readwrite');
      tx.objectStore('renders').put({ id, hash, images });
      tx.oncomplete = res; tx.onerror = res; tx.onabort = res;
    });
  }
  async delete(id) {
    const db = await this._open();
    if (!db) return;
    await new Promise((res) => { const tx = db.transaction('renders', 'readwrite'); tx.objectStore('renders').delete(id); tx.oncomplete = res; tx.onerror = res; });
  }
}

export const app = {
  shared: null,
  settings: loadSettings(),
  store: new ProjectStore('indexedDB' in globalThis ? new IDBBackend() : new MemoryBackend()),
  renderCache: new RenderCache(),
  project: null, src: null, srcCanvas: null, renders: {}, rendersHash: null,
  aiKeySession: null,

  saveSettings() {
    const { ...s } = this.settings;
    safeSet(localStorage, SETTINGS_KEY, JSON.stringify(s));
    applySettings(this.settings);
  },
  get aiKey() {
    return this.aiKeySession || safeGet(localStorage, 'drawstep.aikey') || safeGet(sessionStorage, 'drawstep.aikey') || '';
  },
  setAiKey(key, remember) {
    this.aiKeySession = key || null;
    safeDel(localStorage, 'drawstep.aikey'); safeDel(sessionStorage, 'drawstep.aikey');
    if (key) safeSet(remember ? localStorage : sessionStorage, 'drawstep.aikey', key);
  },

  async setProject(p, { src = null } = {}) {
    if (!this.project || this.project.id !== p.id) { this.renders = {}; this.rendersHash = null; }
    this.project = p;
    if (src) this.src = src;
    else if (!this.src || this._srcFor !== p.id) this.src = await sourceToRGBA(p.source);
    this._srcFor = p.id;
    this.srcCanvas = rgbaToCanvas(this.src);
  },
  async saveProject() {
    if (!this.project) return;
    try { this.project = await this.store.save(this.project); }
    catch (e) { throw new Error('Could not save to this device. Storage may be full or disabled. ' + (e.message || '')); }
  },
  async openProject(id) {
    const p = await this.store.get(id);
    if (!p) throw new Error('That project no longer exists.');
    await this.setProject(p);
    return p;
  },
  async loadDemo() {
    const res = await fetch('demo/demo-project.drawstep.json');
    if (!res.ok) throw new Error('The demo project could not be loaded.');
    const p = importProject(await res.text(), { newId: true });
    p.createdAt = p.updatedAt = new Date().toISOString();
    await this.setProject(p);
    const { thumbnail } = await import('./ui/imageio.js');
    p.thumb = thumbnail(this.srcCanvas);
    await this.saveProject();
    return p;
  },
  /** Do we have up-to-date renders for the current prep (memory or IndexedDB)? */
  async loadCachedRenders() {
    const hash = prepHash(this.project.prep);
    if (this.rendersHash === hash && Object.keys(this.renders).length) return true;
    const cached = await this.renderCache.get(this.project.id, hash).catch(() => null);
    if (cached && cached[this.project.prep.style]) { this.renders = cached; this.rendersHash = hash; return true; }
    return false;
  },
  async setRenders(rgbaMap) {
    this.renders = {};
    for (const [k, r] of Object.entries(rgbaMap)) this.renders[k] = rgbaToCanvas(r);
    this.rendersHash = prepHash(this.project.prep);
    this.renderCache.put(this.project.id, this.rendersHash, this.renders).catch(() => {});
  },
  /** Raster for a step base ('light'|'outline'|'values'|'sketch'|'none') or a style id. */
  baseCanvas(base) {
    if (base === 'none') return null;
    if (base === 'reference' || base === 'original') return this.renders.original || this.srcCanvasPrepared();
    return this.renders[base] || this.renders[this.project.prep.style] || null;
  },
  srcCanvasPrepared() { return this.renders.original || this.srcCanvas; },
};

export function applySettings(s) {
  const root = document.documentElement;
  if (s.theme === 'system') root.removeAttribute('data-theme'); else root.setAttribute('data-theme', s.theme);
  if (s.motion === 'system') root.removeAttribute('data-motion'); else root.setAttribute('data-motion', s.motion);
  root.style.setProperty('--text-scale', String(s.textScale));
}

export const reducedMotion = () => app.settings.motion === 'reduce' || (app.settings.motion === 'system' && matchMedia('(prefers-reduced-motion: reduce)').matches);
