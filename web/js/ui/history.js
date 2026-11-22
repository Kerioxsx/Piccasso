// Drawing history dialog: dated photos of your drawing, before/after compare,
// and the project's cover picture. Everything stays in the project (on the device).
import { app } from '../state.js';
import { h, toast, dialog, pickFile, confirmDialog } from './dom.js';
import { fileToCanvas } from './imageio.js';
import { addPhoto, removePhoto, setNote, coverOf, beforeAfter, gapText } from '../core/history.js';

const fmt = (iso) => new Date(iso).toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' });

async function fileToDataUrl(file, max) {
  const c = await fileToCanvas(file);
  const k = Math.min(1, max / Math.max(c.width, c.height));
  const o = document.createElement('canvas');
  o.width = Math.round(c.width * k); o.height = Math.round(c.height * k);
  const g = o.getContext('2d');
  g.fillStyle = '#fff'; g.fillRect(0, 0, o.width, o.height); // transparent pictures get white paper, not black
  g.drawImage(c, 0, 0, o.width, o.height);
  return o.toDataURL('image/jpeg', 0.82);
}

/** Opens the dialog for project `p` (a stored project). Resolves when closed. */
export async function openHistory(p) {
  let proj = await app.store.get(p.id) || p;
  const persist = async (patch) => {
    proj = await app.store.update(proj.id, patch);
    if (app.project && app.project.id === proj.id) Object.assign(app.project, patch);
  };
  const body = h('div.stack.history');
  const draw = () => {
    const list = proj.history || [];
    const coverImg = h('img.history-cover', { src: coverOf(proj), alt: 'Current cover' });
    const coverRow = h('div.row', { style: { alignItems: 'center' } }, coverImg,
      h('div.stack', { style: { gap: '6px' } }, h('strong', 'Cover'),
        h('div.row', { style: { gap: '6px' } },
          h('button.btn.small', { type: 'button', disabled: !proj.cover, on: { click: async () => { await persist({ cover: null }); draw(); } } }, 'Use the reference picture'),
          h('button.btn.small', { type: 'button', on: { click: async () => {
            const f = await pickFile('image/*'); if (!f) return;
            try { await persist({ cover: await fileToDataUrl(f, 320) }); draw(); } catch (e) { toast(e.message, { error: true }); }
          } } }, 'Choose a picture…'))));
    const add = h('button.btn.primary', { type: 'button', on: { click: async () => {
      const f = await pickFile('image/*', 'environment'); if (!f) return;
      try { const next = addPhoto(proj, { image: await fileToDataUrl(f, 1000) }); await persist({ history: next.history }); toast('Photo added to the history'); draw(); }
      catch (e) { toast(e.message, { error: true }); }
    } } }, 'Add a photo of my drawing');

    const parts = [coverRow, h('div.row', add, h('span.small.muted', list.length ? `${list.length} photo${list.length > 1 ? 's' : ''}` : 'Take a photo of your drawing now and then to see how it grows.'))];

    const pair = beforeAfter(proj);
    if (pair) {
      const opts = (sel) => list.map((e, i) => h('option', { value: e.id, selected: e.id === sel }, `${i + 1}. ${fmt(e.at)}`));
      const aSel = h('select', { 'aria-label': 'Before' }, ...opts(pair[0]));
      const bSel = h('select', { 'aria-label': 'After' }, ...opts(pair[1]));
      const before = h('img', { alt: 'Before' }), after = h('img', { alt: 'After' });
      const top = h('div.ba-top', after);
      const gap = h('span.small.muted');
      const slider = h('input', { type: 'range', min: 0, max: 100, value: 50, 'aria-label': 'Before / after divider', on: { input: () => { top.style.clipPath = `inset(0 0 0 ${slider.value}%)`; } } });
      const sync = () => {
        const a = list.find((e) => e.id === aSel.value), b = list.find((e) => e.id === bSel.value);
        before.src = a.image; after.src = b.image; gap.textContent = gapText(a.at, b.at);
        top.style.clipPath = `inset(0 0 0 ${slider.value}%)`;
      };
      aSel.addEventListener('change', sync); bSel.addEventListener('change', sync);
      parts.push(h('div.stack', { style: { gap: '6px' } },
        h('div.row', h('strong', 'Before / after'), h('label.row', { style: { gap: '4px' } }, 'Before', aSel), h('label.row', { style: { gap: '4px' } }, 'After', bSel), gap),
        h('div.ba', before, top, h('span.ba-label', 'before'), h('span.ba-label.r', 'after')), slider));
      sync();
    }

    if (list.length) {
      parts.push(h('div.history-grid', ...list.slice().reverse().map((e) => {
        const isCover = proj.cover === 'photo:' + e.id;
        const note = h('input', { type: 'text', value: e.note || '', placeholder: 'Note', 'aria-label': `Note for the photo from ${fmt(e.at)}`, on: { change: async () => { await persist({ history: setNote(proj, e.id, note.value).history }); } } });
        return h('figure.history-item', h('img', { src: e.image, alt: `Drawing on ${fmt(e.at)}` }),
          h('figcaption.small', fmt(e.at)), note,
          h('div.row', { style: { gap: '4px' } },
            h('button.btn.small', { type: 'button', 'aria-pressed': String(isCover), on: { click: async () => { await persist({ cover: isCover ? null : 'photo:' + e.id }); draw(); } } }, isCover ? 'Cover ✓' : 'Make cover'),
            h('button.btn.small.danger', { type: 'button', 'aria-label': `Delete the photo from ${fmt(e.at)}`, on: { click: async () => {
              if (!(await confirmDialog('Delete this photo?', 'It will be removed from the history.', 'Delete', true))) return;
              const next = removePhoto(proj, e.id); await persist({ history: next.history, cover: next.cover }); draw();
            } } }, 'Delete')));
      })));
    }
    body.replaceChildren(...parts);
  };
  draw();
  await dialog({ title: `History of “${proj.name}”`, body, buttons: [{ label: 'Close', value: true, primary: true }] });
}
