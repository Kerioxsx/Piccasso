// Skill tree: ten categories of lessons, unlocked step by step; each lesson has
// a goal, numbered steps, a button that opens the right tool, notes and a
// bookmark. Progress, notes and bookmarks stay on this device.
import { h, nav, toast } from '../dom.js';
import { isUnlocked, nextLesson, categoryProgress, milestones, emptyProgress, LEVELS } from '../../core/skills.js';

const KEY = 'piccasso.skills';
const load = () => { try { return { ...emptyProgress(), ...JSON.parse(localStorage.getItem(KEY)) }; } catch { return emptyProgress(); } };
const save = (p) => { try { localStorage.setItem(KEY, JSON.stringify(p)); } catch { /* storage off */ } };
const LEVEL_TEXT = { beginner: 'Beginner', intermediate: 'Intermediate', advanced: 'Advanced' };

let cache = null;
async function lessons() { if (!cache) cache = await (await fetch('shared/lessons.en.json')).json(); return cache; }

export async function renderSkills(main) {
  const data = await lessons();
  let progress = load();
  let filter = 'all';
  const root = h('div.skills');
  main.appendChild(root);
  const byId = Object.fromEntries(data.lessons.map((l) => [l.id, l]));

  function status(l) { return progress.done[l.id] ? 'done' : isUnlocked(l, progress) ? 'open' : 'locked'; }
  const mark = { done: '✓', open: '○', locked: '🔒' };

  function renderTree() {
    const cats = categoryProgress(data, progress);
    const total = Object.keys(progress.done).length;
    const nxt = nextLesson(data, progress);
    const visible = (l) => (filter === 'all' || l.level === filter || (filter === 'saved' && progress.bookmarks.includes(l.id)));
    const ms = milestones(data, progress);
    root.replaceChildren(h('div.stack',
      h('div.row', { style: { justifyContent: 'space-between', alignItems: 'end' } },
        h('div', h('h1', 'Skill tree'), h('p.muted', `${total} of ${data.lessons.length} lessons done. Lessons open up as you finish the ones before them.`)),
        h('a.btn.small', { href: '#/learn' }, 'Drawing words')),
      nxt ? h('section.card.skills-next', h('div', h('span.small.muted', 'Next up'), h('h2', nxt.title), h('p', nxt.goal)),
        h('button.btn.primary', { type: 'button', on: { click: () => renderLesson(nxt.id) } }, 'Start lesson')) : h('section.card', h('h2', 'Every lesson done!'), h('p', 'Keep practising with timed poses and your own projects.')),
      h('div.chips', { role: 'group', 'aria-label': 'Filter lessons' },
        ...[['all', 'All'], ...LEVELS.map((l) => [l, LEVEL_TEXT[l]]), ['saved', `Saved (${progress.bookmarks.length})`]].map(([k, t]) =>
          h('button.chip', { type: 'button', 'aria-pressed': String(filter === k), on: { click: () => { filter = k; renderTree(); } } }, t))),
      h('div.skills-grid', ...cats.map((c, ci) => {
        const ls = data.lessons.filter((l) => l.cat === c.id && visible(l));
        if (!ls.length && filter !== 'all') return null;
        return h('section.card.skills-cat',
          h('div.skills-cat-head', h('span.skills-num', String(ci + 1)), h('div', h('h2', c.title), h('p.small.muted', c.summary))),
          h('div.skills-bar', { role: 'progressbar', 'aria-label': `${c.title} progress`, 'aria-valuenow': Math.round(c.fraction * 100), 'aria-valuemin': 0, 'aria-valuemax': 100 }, h('i', { style: { width: `${c.fraction * 100}%` } })),
          h('ol.skills-lessons', ...ls.map((l) => {
            const s = status(l);
            return h('li', h('button.skills-lesson', { type: 'button', 'data-status': s, on: { click: () => renderLesson(l.id) }, 'aria-label': `${l.title}, ${LEVEL_TEXT[l.level]}, ${s === 'done' ? 'done' : s === 'open' ? 'open' : 'locked'}` },
              h('span.skills-mark', { 'aria-hidden': 'true' }, mark[s]), h('span', l.title),
              h('span.small.muted', `${LEVEL_TEXT[l.level]} · ${l.minutes} min${progress.bookmarks.includes(l.id) ? ' · ★' : ''}`)));
          })));
      })),
      h('section.card.stack', h('h2', 'Milestones'),
        ms.length ? h('div.chips', ...ms.map((m) => h('span.chip', { 'aria-pressed': 'true' }, '★ ', m.title))) : h('p.small.muted', 'Finish your first lesson to earn a milestone.')),
      h('section.card.stack', h('h2', 'Learning history'), historyList())));
  }

  function historyList() {
    const items = Object.entries(progress.done).sort((a, b) => b[1].localeCompare(a[1])).slice(0, 12);
    if (!items.length) return h('p.small.muted', 'Nothing yet.');
    return h('ul.practice-history', ...items.map(([id, at]) => h('li', h('span', byId[id] ? byId[id].title : id), h('span.small.muted', new Date(at).toLocaleDateString()))));
  }

  function renderLesson(id) {
    const l = byId[id];
    const s = status(l);
    const cat = data.categories.find((c) => c.id === l.cat);
    const notes = h('textarea', { rows: 4, 'aria-label': 'My notes', placeholder: 'What worked, what to try next time…', value: progress.notes[l.id] || '',
      on: { input: (e) => { progress.notes[l.id] = e.target.value; save(progress); } } });
    const saved = progress.bookmarks.includes(l.id);
    root.replaceChildren(h('div.stack', { style: { maxWidth: '760px' } },
      h('button.btn.small.ghost', { type: 'button', style: { alignSelf: 'start' }, on: { click: renderTree } }, '← Skill tree'),
      h('span.small.muted', `${cat.title} · ${LEVEL_TEXT[l.level]} · about ${l.minutes} minutes`),
      h('h1', l.title),
      h('p.lead', l.goal),
      s === 'locked' ? h('div.notice', 'First finish: ', ...l.requires.filter((r) => !progress.done[r]).map((r, i) => [i ? ', ' : '', h('a', { href: '#', on: { click: (e) => { e.preventDefault(); renderLesson(r); } } }, byId[r].title)])) : null,
      h('ol.skills-steps', ...l.steps.map((t, i) => h('li', h('label', h('input', { type: 'checkbox', 'aria-label': `Step ${i + 1} done` }), h('span', t))))),
      l.tool ? h('button.btn', { type: 'button', style: { alignSelf: 'start' }, on: { click: () => nav(l.tool.route) } }, l.tool.label) : null,
      h('label.field', 'My notes', notes),
      h('div.row',
        progress.done[l.id]
          ? h('button.btn', { type: 'button', on: { click: () => { delete progress.done[l.id]; save(progress); renderLesson(id); } } }, `Done on ${new Date(progress.done[l.id]).toLocaleDateString()} · undo`)
          : h('button.btn.primary', { type: 'button', disabled: s === 'locked', on: { click: () => {
            const before = milestones(data, progress).length;
            progress.done[l.id] = new Date().toISOString(); save(progress);
            const ms = milestones(data, progress);
            if (ms.length > before) toast(`Milestone: ${ms[ms.length - 1].title}`); else toast('Lesson done!');
            const n = nextLesson(data, progress);
            if (n) renderLesson(n.id); else renderTree();
          } } }, 'Mark as done'),
        h('button.btn', { type: 'button', 'aria-pressed': String(saved), on: { click: () => {
          progress.bookmarks = saved ? progress.bookmarks.filter((b) => b !== l.id) : [...progress.bookmarks, l.id]; save(progress); renderLesson(id);
        } } }, saved ? '★ Saved to my practice set' : '☆ Save to my practice set'))));
    window.scrollTo(0, 0);
  }

  renderTree();
}
