import test from 'node:test';
import assert from 'node:assert/strict';
import { ProjectStore, MemoryBackend } from '../js/core/store.js';

test('library metadata updates without moving the project to the top', async () => {
  const s = new ProjectStore(new MemoryBackend());
  await s.save({ id: 'a', name: 'A', steps: [], progress: {} });
  await new Promise((r) => setTimeout(r, 5));
  await s.save({ id: 'b', name: 'B', steps: [], progress: {} });
  const before = (await s.get('a')).updatedAt;
  await s.update('a', { library: { folder: 'Portraits', tags: ['face'], favorite: true, notes: 'x' } });
  const a = await s.get('a');
  assert.deepEqual(a.library, { folder: 'Portraits', tags: ['face'], favorite: true, notes: 'x' });
  assert.equal(a.updatedAt, before, 'recent order unchanged');
  assert.equal((await s.list())[0].id, 'b');
  await assert.rejects(() => s.update('nope', {}));
});
