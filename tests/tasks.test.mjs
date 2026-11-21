import assert from 'node:assert/strict';
import { test } from 'node:test';
import { groupTasks, isOverdue, moveTask, taskSection, taskStatus } from '../node_modules/.cache/kairos-tests/taskModel.js';

const now = new Date(2026, 10, 21, 14, 30);
const task = (id, changes = {}) => ({ id, priority: 'medium', manualOrder: 0, ...changes });

test('scheduled dates and deadline timestamps classify tasks independently', () => {
  const sameDay = task('same', { dueDate: '2026-11-21' });
  assert.equal(taskSection(sameDay, now), 'No Date');
  assert.equal(isOverdue(sameDay, new Date(2026, 10, 21, 23, 58, 59)), false);
  assert.equal(isOverdue(sameDay, new Date(2026, 10, 21, 23, 59)), true);
  const missed = task('missed', { scheduledDate: '2026-11-20', dueDate: '2026-11-24' });
  assert.equal(taskSection(missed, now), 'Today');
  assert.equal(taskStatus(missed, now), 'Rescheduled');
  const late = task('late', { scheduledDate: '2026-11-24', dueDate: '2026-11-20' });
  assert.equal(taskSection(late, now), 'Today');
  assert.equal(taskStatus(late, now), 'Overdue 1 day');
  assert.equal(isOverdue(task('done', { ...late, completedAt: '2026-11-20T09:00:00Z' }), now), false);
});

test('automatic ordering prioritizes overdue, scheduled dates, deadlines, then priority', () => {
  const tasks = [
    task('undated', { scheduledDate: '2026-11-21', priority: 'high' }),
    task('medium', { scheduledDate: '2026-11-21', dueDate: '2026-11-22' }),
    task('late', { dueDate: '2026-11-20' }),
    task('high', { scheduledDate: '2026-11-21', dueDate: '2026-11-22', priority: 'high' }),
    task('later', { scheduledDate: '2026-11-27', dueDate: '2026-11-24' }),
    task('sooner', { scheduledDate: '2026-11-23' }),
  ];
  const grouped = groupTasks(tasks, 'automatic', now);
  assert.deepEqual(grouped.Today.map(({ id }) => id), ['late', 'high', 'medium', 'undated']);
  assert.deepEqual(grouped.Upcoming.map(({ id }) => id), ['sooner', 'later']);
});

test('manual ordering preserves the overdue group and supports keyboard and drag moves', () => {
  const tasks = [task('last', { manualOrder: 20 }), task('first', { manualOrder: -2 })];
  const grouped = groupTasks(tasks, 'manual', now);
  assert.deepEqual(grouped['No Date'].map(({ id }) => id), ['first', 'last']);
  assert.deepEqual(moveTask(grouped['No Date'], 'first', 'last', now), ['last', 'first']);
  assert.equal(moveTask([task('late', { dueDate: '2026-11-20' }), tasks[0]], 'last', 'late', now), null);
});
