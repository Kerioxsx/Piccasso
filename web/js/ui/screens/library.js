import { app } from '../../state.js';
import { h, icon, toast, confirmDialog, promptDialog } from '../dom.js';
import { projectCard, importProjectFile } from './home.js';
import { exportProjectFile } from '../exporter.js';

export async function renderLibrary(main) {
  const list = h('div.project-list');
  main.appendChild(h('div.stack',
    h('div.row', h('h1', { style: { margin: 0 } }, 'Your projects'),
      h('button.btn.small', { type: 'button', style: { marginLeft: 'auto' }, on: { click: importProjectFile } }, icon('file'), 'Import project file')),
    h('p.small.muted', 'Projects are saved on this device only. Export a project file to back it up or open it in the DrawStep desktop app.'),
    list));

  async function refresh() {
    list.innerHTML = '';
    let projects = [];
    try { projects = await app.store.list(); } catch (e) { list.appendChild(h('div.notice', 'Could not read saved projects: ' + e.message)); return; }
    if (!projects.length) { list.appendChild(h('div.empty', 'No projects yet. ', h('a', { href: '#/home' }, 'Start one'))); return; }
    for (const p of projects) {
      const actions = h('div.row', { style: { gap: '4px', marginTop: '6px' } },
        h('button.btn.small.ghost', { type: 'button', 'aria-label': `Rename ${p.name}`, on: { click: async () => {
          const name = await promptDialog('Rename project', p.name);
          if (name && name.trim()) { await app.store.rename(p.id, name); if (app.project && app.project.id === p.id) app.project.name = name.trim(); refresh(); }
        } } }, icon('edit')),
        h('button.btn.small.ghost', { type: 'button', 'aria-label': `Duplicate ${p.name}`, on: { click: async () => { await app.store.duplicate(p.id); toast('Project duplicated'); refresh(); } } }, icon('copy')),
        h('button.btn.small.ghost', { type: 'button', 'aria-label': `Export ${p.name}`, on: { click: () => exportProjectFile(p) } }, icon('export')),
        h('button.btn.small.ghost.danger', { type: 'button', 'aria-label': `Delete ${p.name}`, on: { click: async () => {
          if (!(await confirmDialog('Delete project?', `“${p.name}” will be removed from this device. This can't be undone.`, 'Delete', true))) return;
          await app.store.delete(p.id); await app.renderCache.delete(p.id);
          if (app.project && app.project.id === p.id) { app.project = null; sessionStorage.removeItem('drawstep.current'); }
          toast('Project deleted'); refresh();
        } } }, icon('trash')));
      list.appendChild(projectCard(p, actions));
    }
  }
  await refresh();
}
