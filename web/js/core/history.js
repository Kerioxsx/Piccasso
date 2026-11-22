// Drawing history: dated photos of your drawing kept in the project, a
// before/after pair, and the project's cover picture.
import { uuid } from './project.js';

export const MAX_HISTORY = 40;

/** Add a photo entry (newest last); keeps at most MAX_HISTORY, dropping the oldest except the first ("before"). */
export function addPhoto(p, { image, note = '', at = new Date().toISOString() }) {
  const list = [...(p.history || []), { id: uuid(), at, image, note }];
  while (list.length > MAX_HISTORY) list.splice(1, 1);
  return { ...p, history: list };
}

export function removePhoto(p, id) {
  const history = (p.history || []).filter((e) => e.id !== id);
  const cover = p.cover === 'photo:' + id ? null : p.cover ?? null;
  return { ...p, history, cover };
}

export function setNote(p, id, note) { return { ...p, history: (p.history || []).map((e) => (e.id === id ? { ...e, note } : e)) }; }

/** The image to show for the project: a chosen photo, a custom picture, or the reference thumbnail. */
export function coverOf(p) {
  const c = p.cover;
  if (typeof c === 'string' && c.startsWith('photo:')) { const e = (p.history || []).find((x) => 'photo:' + x.id === c); if (e) return e.image; }
  if (typeof c === 'string' && c.startsWith('data:')) return c;
  return p.thumb || '';
}

/** Default before/after pair: first and latest photo (ids), or null with fewer than two. */
export function beforeAfter(p) {
  const h = p.history || [];
  return h.length >= 2 ? [h[0].id, h[h.length - 1].id] : null;
}

/** "3 days later" style gap between two ISO dates. */
export function gapText(a, b) {
  const days = Math.round((new Date(b) - new Date(a)) / 86400000);
  if (days <= 0) return 'same day';
  if (days === 1) return '1 day later';
  if (days < 60) return `${days} days later`;
  return `${Math.round(days / 30)} months later`;
}
