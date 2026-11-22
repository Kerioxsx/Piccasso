import { app } from '../../state.js';
import { h, icon, toast, confirmDialog, promptDialog, dialog } from '../dom.js';
import { projectCard, importProjectFile } from './home.js';
import { exportProjectFile } from '../exporter.js';
import { openHistory } from '../history.js';

/** Library metadata kept in the project file: folder, tags, favourite, notes. */
const lib = (p) => ({ folder: '', tags: [], favorite: false, notes: '', ...(p.library || {}) });
/** Save library metadata; the open project in memory gets it too, so a later save keeps it. */
async function setLib(p, library) {
  await app.store.update(p.id, { library });
  if (app.project && app.project.id === p.id) app.project.library = library;
}

export async function renderLibrary(main) {
  const list = h('div.project-list');
  const filters = h('div.chips', { role: 'group', 'aria-label': 'Filter projects' });
  const search = h('input.lib-search', { type: 'search', placeholder: 'Search names, tags and notes', 'aria-label': 'Search projects', on: { input: () => draw() } });
  const sortSel = h('select', { 'aria-label': 'Sort projects', on: { change: () => draw() } }, h('option', { value: 'recent' }, 'Recent first'), h('option', { value: 'name' }, 'Name A–Z'), h('option', { value: 'progress' }, 'Least finished first'));
  let filter = 'all';
  let projects = [];
  main.appendChild(h('div.stack',
    h('div.row', h('h1', { style: { margin: 0 } }, 'Your projects'),
      h('button.btn.small', { type: 'button', style: { marginLeft: 'auto' }, on: { click: importProjectFile } }, icon('file'), 'Import project file')),
    h('p.small.muted', 'Projects are saved on this device only. Export a project file to back it up or open it in the Piccasso desktop app.'),
    h('div.row', search, sortSel),
    filters,
    list));

  async function edit(p) {
    const m = lib(p);
    const folders = [...new Set(projects.map((x) => lib(x).folder).filter(Boolean))];
    const folder = h('input', { type: 'text', value: m.folder, list: 'lib-folders', 'aria-label': 'Folder' });
    const tags = h('input', { type: 'text', value: m.tags.join(', '), 'aria-label': 'Tags', placeholder: 'portrait, practice, gift' });
    const notes = h('textarea', { rows: 4, 'aria-label': 'Notes', value: m.notes });
    const ok = await dialog({ title: `Organise “${p.name}”`, body: h('div.stack',
      h('label.field', 'Folder', folder, h('datalist', { id: 'lib-folders' }, ...folders.map((f) => h('option', { value: f })))),
      h('label.field', 'Tags (separate with commas)', tags),
      h('label.field', 'Notes', notes)),
    buttons: [{ label: 'Cancel', value: false }, { label: 'Save', value: true, primary: true }] });
    if (!ok) return;
    await setLib(p, { ...m, folder: folder.value.trim(), tags: [...new Set(tags.value.split(',').map((t) => t.trim().toLowerCase()).filter(Boolean))], notes: notes.value });
    refresh();
  }

  function draw() {
    const folders = [...new Set(projects.map((p) => lib(p).folder).filter(Boolean))].sort();
    const tags = [...new Set(projects.flatMap((p) => lib(p).tags))].sort();
    filters.replaceChildren(...[['all', 'All'], ['fav', '★ Favourites'], ...folders.map((f) => [`f:${f}`, `📁 ${f}`]), ...tags.map((t) => [`t:${t}`, `#${t}`])].map(([k, l]) =>
      h('button.chip', { type: 'button', 'aria-pressed': String(filter === k), on: { click: () => { filter = filter === k ? 'all' : k; draw(); } } }, l)));
    const q = search.value.trim().toLowerCase();
    const pct = (p) => (p.steps && p.steps.length ? (p.progress.completed.length + p.progress.skipped.length) / p.steps.length : 0);
    let shown = projects.filter((p) => {
      const m = lib(p);
      if (filter === 'fav' && !m.favorite) return false;
      if (filter.startsWith('f:') && m.folder !== filter.slice(2)) return false;
      if (filter.startsWith('t:') && !m.tags.includes(filter.slice(2))) return false;
      return !q || [p.name, m.folder, m.notes, ...m.tags].join(' ').toLowerCase().includes(q);
    });
    if (sortSel.value === 'name') shown = shown.slice().sort((a, b) => a.name.localeCompare(b.name));
    else if (sortSel.value === 'progress') shown = shown.slice().sort((a, b) => pct(a) - pct(b));
    else shown = shown.slice().sort((a, b) => (lib(b).favorite - lib(a).favorite) || String(b.updatedAt || '').localeCompare(String(a.updatedAt || '')));
    list.innerHTML = '';
    if (!projects.length) { list.appendChild(h('div.empty', 'No projects yet. ', h('a', { href: '#/home' }, 'Start one'))); return; }
    if (!shown.length) { list.appendChild(h('div.empty', 'No projects match.')); return; }
    for (const p of shown) {
      const m = lib(p);
      const info = (m.folder || m.tags.length) ? h('div.small.muted', [m.folder ? `📁 ${m.folder}` : '', ...m.tags.map((t) => `#${t}`)].filter(Boolean).join('  ')) : null;
      const actions = h('div.row', { style: { gap: '4px', marginTop: '6px' } },
        h('button.btn.small.ghost', { type: 'button', 'aria-pressed': String(m.favorite), 'aria-label': `${m.favorite ? 'Remove from' : 'Add to'} favourites: ${p.name}`, on: { click: async () => { await setLib(p, { ...m, favorite: !m.favorite }); refresh(); } } }, m.favorite ? '★' : '☆'),
        h('button.btn.small.ghost', { type: 'button', 'aria-label': `Folder, tags and notes for ${p.name}`, on: { click: () => edit(p) } }, '🏷'),
        h('button.btn.small.ghost', { type: 'button', 'aria-label': `History, before/after and cover for ${p.name}`, title: 'History and cover', on: { click: async () => { await openHistory(p); refresh(); } } }, '🕘'),
        h('button.btn.small.ghost', { type: 'button', 'aria-label': `Rename ${p.name}`, on: { click: async () => {
          const name = await promptDialog('Rename project', p.name);
          if (name && name.trim()) { await app.store.rename(p.id, name); if (app.project && app.project.id === p.id) app.project.name = name.trim(); refresh(); }
        } } }, icon('edit')),
        h('button.btn.small.ghost', { type: 'button', 'aria-label': `Duplicate ${p.name}`, on: { click: async () => { await app.store.duplicate(p.id); toast('Project duplicated'); refresh(); } } }, icon('copy')),
        h('button.btn.small.ghost', { type: 'button', 'aria-label': `Export ${p.name}`, on: { click: () => exportProjectFile(p) } }, icon('export')),
        h('button.btn.small.ghost.danger', { type: 'button', 'aria-label': `Delete ${p.name}`, on: { click: async () => {
          if (!(await confirmDialog('Delete project?', `“${p.name}” will be removed from this device. This can't be undone.`, 'Delete', true))) return;
          await app.store.delete(p.id); await app.renderCache.delete(p.id);
          if (app.project && app.project.id === p.id) { app.project = null; sessionStorage.removeItem('piccasso.current'); }
          toast('Project deleted'); refresh();
        } } }, icon('trash')));
      list.appendChild(projectCard(p, h('div', info, m.notes ? h('div.small', { style: { whiteSpace: 'pre-line' } }, m.notes.slice(0, 140)) : null, actions)));
    }
  }

  async function refresh() {
    try { projects = await app.store.list(); } catch (e) { list.replaceChildren(h('div.notice', 'Could not read saved projects: ' + e.message)); return; }
    draw();
  }
  await refresh();
}
