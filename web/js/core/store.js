// Project storage. ProjectStore holds the app-level rules (sorting, duplicate,
// rename, timestamps); a backend does the persistence. IDBBackend is used in the
// browser (IndexedDB); MemoryBackend is used in tests.

import { uuid } from './project.js';

export class MemoryBackend {
  constructor() { this.map = new Map(); }
  async getAll() { return [...this.map.values()].map((v) => structuredClone(v)); }
  async get(id) { const v = this.map.get(id); return v ? structuredClone(v) : null; }
  async put(p) { this.map.set(p.id, structuredClone(p)); }
  async delete(id) { this.map.delete(id); }
}

export class IDBBackend {
  constructor(name = 'drawstep', store = 'projects') { this.name = name; this.store = store; this._db = null; }
  _open() {
    if (this._db) return this._db;
    this._db = new Promise((resolve, reject) => {
      const req = indexedDB.open(this.name, 1);
      req.onupgradeneeded = () => {
        const db = req.result;
        if (!db.objectStoreNames.contains(this.store)) db.createObjectStore(this.store, { keyPath: 'id' });
      };
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error || new Error('Could not open local storage (IndexedDB).'));
    });
    return this._db;
  }
  async _tx(mode, fn) {
    const db = await this._open();
    return new Promise((resolve, reject) => {
      const tx = db.transaction(this.store, mode);
      const st = tx.objectStore(this.store);
      const req = fn(st);
      tx.oncomplete = () => resolve(req ? req.result : undefined);
      tx.onerror = () => reject(tx.error);
      tx.onabort = () => reject(tx.error || new Error('Storage transaction aborted (storage may be full).'));
    });
  }
  getAll() { return this._tx('readonly', (s) => s.getAll()); }
  async get(id) { return (await this._tx('readonly', (s) => s.get(id))) || null; }
  put(p) { return this._tx('readwrite', (s) => { s.put(p); }); }
  delete(id) { return this._tx('readwrite', (s) => { s.delete(id); }); }
}

export class ProjectStore {
  constructor(backend) { this.backend = backend; }
  /** Summaries, most recently updated first. */
  async list() {
    const all = await this.backend.getAll();
    return all.sort((a, b) => (b.updatedAt || '').localeCompare(a.updatedAt || ''));
  }
  get(id) { return this.backend.get(id); }
  async save(p, { touch = true } = {}) {
    const copy = { ...p };
    if (touch) copy.updatedAt = new Date().toISOString();
    for (const k of Object.keys(copy)) if (k.startsWith('_')) delete copy[k];
    await this.backend.put(copy);
    return copy;
  }
  async rename(id, name) {
    const p = await this.get(id);
    if (!p) throw new Error('Project not found.');
    return this.save({ ...p, name: name.trim() || p.name });
  }
  async duplicate(id) {
    const p = await this.get(id);
    if (!p) throw new Error('Project not found.');
    const now = new Date().toISOString();
    return this.save({ ...structuredClone(p), id: uuid(), name: `${p.name} (copy)`, createdAt: now }, { touch: true });
  }
  delete(id) { return this.backend.delete(id); }
}
