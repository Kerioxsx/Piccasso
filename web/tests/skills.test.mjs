import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { validate, isUnlocked, nextLesson, categoryProgress, milestones, emptyProgress } from '../js/core/skills.js';

const data = JSON.parse(fs.readFileSync(new URL('../../shared/lessons.en.json', import.meta.url)));

test('lesson data is consistent: 10 categories, every lesson reachable', () => {
  assert.equal(data.categories.length, 10);
  assert.deepEqual(validate(data), []);
  for (const c of data.categories) assert.ok(data.lessons.some((l) => l.cat === c.id), c.id);
  for (const l of data.lessons) if (l.tool) assert.ok(['sandbox', 'guides', 'studio', 'practice', 'home'].includes(l.tool.route), l.id);
});

test('unlocking, recommendation and milestones', () => {
  const p = emptyProgress();
  const boxes = data.lessons.find((l) => l.id === 'shapes-boxes');
  assert.equal(isUnlocked(boxes, p), false);
  const first = nextLesson(data, p);
  assert.equal(first.level, 'beginner');
  p.done['shapes-circles'] = '2026-11-21';
  assert.equal(isUnlocked(boxes, p), true);
  assert.deepEqual(milestones(data, p).map((m) => m.id), ['first']);
  for (const l of data.lessons) p.done[l.id] = '2026-11-21';
  const ms = milestones(data, p).map((m) => m.id);
  assert.ok(ms.includes('all') && ms.includes('basics') && ms.includes('cat-shapes'));
  assert.equal(nextLesson(data, p), null);
  assert.ok(categoryProgress(data, p).every((c) => c.fraction === 1));
  assert.ok(validate({ categories: [{ id: 'a' }], lessons: [{ id: 'x', cat: 'a', level: 'beginner', steps: ['s'], requires: ['y'] }, { id: 'y', cat: 'a', level: 'beginner', steps: ['s'], requires: ['x'] }] }).length > 0, 'cycles are caught');
});
