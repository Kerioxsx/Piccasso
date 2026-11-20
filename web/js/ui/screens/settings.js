import { app, DEFAULT_SETTINGS } from '../../state.js';
import { h, toggle, segmented, slider, toast, confirmDialog } from '../dom.js';
import { WakeLock } from '../wakelock.js';
import { speechSupported } from '../speech.js';
import { probeAr } from '../../core/capabilities.js';

export async function renderSettings(main) {
  const s = app.settings;
  const set = (k, v) => { s[k] = v; app.saveSettings(); };
  const keyInput = h('input', { type: 'password', value: app.aiKey, placeholder: 'sk-ant-…', autocomplete: 'off', 'aria-label': 'Anthropic API key', style: { width: '100%' } });
  const capBox = h('div.small.muted', 'Checking…');
  const storageBox = h('div.small.muted');

  main.appendChild(h('div.stack', { style: { maxWidth: '760px' } },
    h('h1', 'Settings'),
    h('section.card.stack', h('h2', 'Display'),
      segmented('Theme', [['system', 'Match device'], ['light', 'Light'], ['dark', 'Dark']], s.theme, (v) => set('theme', v)),
      slider({ label: 'Instruction text size', min: 0.8, max: 1.8, step: 0.1, value: s.textScale, format: (v) => Math.round(v * 100) + '%', onInput: (v) => set('textScale', v) }),
      segmented('Motion', [['system', 'Match device'], ['reduce', 'Reduce motion'], ['full', 'Allow motion']], s.motion, (v) => set('motion', v))),
    h('section.card.stack', h('h2', 'Guidance'),
      toggle('Read instructions aloud automatically', s.autoRead, (v) => set('autoRead', v), speechSupported() ? 'Uses your device\'s voice.' : 'Not supported in this browser.')),
    h('section.card.stack', h('h2', 'Locked reference'),
      toggle('Keep the screen awake while drawing', s.keepAwake, (v) => set('keepAwake', v), WakeLock.supported ? 'Supported in this browser.' : 'This browser can\'t keep the screen awake. Increase the auto-lock time in your device settings instead.'),
      toggle('Press and hold to unlock', s.holdToUnlock, (v) => set('holdToUnlock', v), 'Prevents accidental unlocking. Turn off if holding is difficult for you.'),
      slider({ label: 'Hold time', min: 600, max: 3000, step: 100, value: s.holdMs, format: (v) => (v / 1000).toFixed(1) + ' s', onInput: (v) => set('holdMs', v) }),
      toggle('Show session timer', s.lockTimer, (v) => set('lockTimer', v))),
    h('section.card.stack', h('h2', 'Camera tracing (AR)'),
      slider({ label: 'Default guide opacity', min: 10, max: 100, value: Math.round(s.arOpacity * 100), format: (v) => v + '%', onInput: (v) => set('arOpacity', v / 100) }),
      capBox),
    h('section.card.stack', h('h2', 'Language'),
      h('label.field', 'App language', h('select', { 'aria-label': 'Language', disabled: true }, h('option', 'English'))),
      h('p.small.muted', 'More languages can be added by translating the shared strings files (see the project README).')),
    h('section.card.stack', h('h2', 'Privacy and AI helper'),
      h('p', 'By default everything happens on this device: pictures, analysis, projects and comparison photos are never uploaded.'),
      toggle('Use the AI helper to write personalised steps', s.aiEnabled, (v) => set('aiEnabled', v),
        'Optional. When on, a smaller copy of your prepared picture and the automatic analysis are sent to Anthropic\'s Claude API with your own API key. You\'ll be asked before anything is sent.'),
      h('label.field', 'Anthropic API key', keyInput),
      toggle('Remember the key on this device', s.rememberKey, (v) => { set('rememberKey', v); app.setAiKey(keyInput.value.trim(), v); }, 'Off: the key is forgotten when you close the browser tab.'),
      h('label.field', 'Model', h('input', { type: 'text', value: s.aiModel, 'aria-label': 'AI model', on: { change: (e) => set('aiModel', e.target.value.trim() || DEFAULT_SETTINGS.aiModel) } })),
      h('div.row',
        h('button.btn.small', { type: 'button', on: { click: () => { app.setAiKey(keyInput.value.trim(), s.rememberKey); toast(keyInput.value.trim() ? 'API key saved' : 'API key removed'); } } }, 'Save key'),
        h('button.btn.small', { type: 'button', on: { click: () => { keyInput.value = ''; app.setAiKey('', false); toast('API key removed'); } } }, 'Remove key'))),
    h('section.card.stack', h('h2', 'Storage'), storageBox,
      h('div.row',
        h('button.btn.small', { type: 'button', on: { click: async () => { if (navigator.storage && navigator.storage.persist) { const ok = await navigator.storage.persist(); toast(ok ? 'Projects will be kept even when space is low.' : 'The browser did not grant persistent storage.'); } else toast('Not supported in this browser.'); } } }, 'Keep my projects safe'),
        h('button.btn.small.danger', { type: 'button', on: { click: async () => {
          if (!(await confirmDialog('Delete all DrawStep data?', 'All projects, cached images and settings on this device will be removed. Export anything you want to keep first.', 'Delete everything', true))) return;
          const all = await app.store.list();
          for (const p of all) { await app.store.delete(p.id); await app.renderCache.delete(p.id); }
          try { localStorage.clear(); sessionStorage.clear(); } catch { /* ignore */ }
          app.project = null;
          toast('All local data deleted');
          location.hash = '#/onboarding'; location.reload();
        } } }, 'Delete all local data'))),
    h('section.card.stack', h('h2', 'About'),
      h('p.small', 'DrawStep web 0.1.0 · Project format v1 · Works offline after the first visit (install it from your browser menu for an app-like experience).'))));

  const cap = await probeAr();
  capBox.textContent = `This device: ${cap.status}. ${cap.reasons.join(' ')}`;
  if (navigator.storage && navigator.storage.estimate) {
    const e = await navigator.storage.estimate();
    storageBox.textContent = `Using about ${(e.usage / 1e6).toFixed(1)} MB of ${(e.quota / 1e6).toFixed(0)} MB available to this site.`;
  }
}
