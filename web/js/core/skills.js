// Skill tree logic: which lessons are open, progress per category, milestones
// and the next recommended lesson. Pure functions over the lesson data and a
// progress record { done: {id: isoDate}, bookmarks: [ids], notes: {id: text} }.

export const LEVELS = ['beginner', 'intermediate', 'advanced'];

/** A lesson is open when every lesson it requires is done. */
export const isUnlocked = (lesson, progress) => (lesson.requires || []).every((r) => progress.done[r]);

export function categoryProgress(data, progress) {
  return data.categories.map((c) => {
    const ls = data.lessons.filter((l) => l.cat === c.id);
    const done = ls.filter((l) => progress.done[l.id]).length;
    return { ...c, total: ls.length, done, fraction: ls.length ? done / ls.length : 0 };
  });
}

/** Next lesson: an open, unfinished lesson, easiest level first, then in the listed order. */
export function nextLesson(data, progress) {
  const open = data.lessons.filter((l) => !progress.done[l.id] && isUnlocked(l, progress));
  open.sort((a, b) => LEVELS.indexOf(a.level) - LEVELS.indexOf(b.level));
  return open[0] || null;
}

/** Milestones reached, in order. */
export function milestones(data, progress) {
  const n = Object.keys(progress.done).length;
  const cats = categoryProgress(data, progress);
  const out = [];
  if (n >= 1) out.push({ id: 'first', title: 'First lesson done' });
  if (n >= 5) out.push({ id: 'five', title: 'Five lessons done' });
  if (n >= 15) out.push({ id: 'fifteen', title: 'Fifteen lessons done' });
  for (const c of cats) if (c.total && c.done === c.total) out.push({ id: `cat-${c.id}`, title: `${c.title} complete` });
  const beginners = data.lessons.filter((l) => l.level === 'beginner');
  if (beginners.length && beginners.every((l) => progress.done[l.id])) out.push({ id: 'basics', title: 'All beginner lessons done' });
  if (n === data.lessons.length && n) out.push({ id: 'all', title: 'Every lesson done' });
  return out;
}

/** Checks lesson data: unique ids, known categories, requirements that exist and no cycles. */
export function validate(data) {
  const errors = [], ids = new Set(), cats = new Set(data.categories.map((c) => c.id));
  for (const l of data.lessons) {
    if (ids.has(l.id)) errors.push(`duplicate ${l.id}`);
    ids.add(l.id);
    if (!cats.has(l.cat)) errors.push(`${l.id}: unknown category ${l.cat}`);
    if (!LEVELS.includes(l.level)) errors.push(`${l.id}: unknown level ${l.level}`);
    if (!l.steps || !l.steps.length) errors.push(`${l.id}: no steps`);
  }
  for (const l of data.lessons) for (const r of l.requires || []) if (!ids.has(r)) errors.push(`${l.id}: requires missing ${r}`);
  // cycles: every lesson must become reachable by completing open lessons in turn
  const done = {};
  let changed = true;
  while (changed) { changed = false; for (const l of data.lessons) if (!done[l.id] && (l.requires || []).every((r) => done[r])) { done[l.id] = true; changed = true; } }
  for (const l of data.lessons) if (!done[l.id]) errors.push(`${l.id}: can never be unlocked`);
  return errors;
}

export const emptyProgress = () => ({ done: {}, bookmarks: [], notes: {} });
