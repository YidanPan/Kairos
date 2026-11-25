import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { OptionalDateTimeInput } from '../node_modules/.cache/kairos-tests/OptionalDateTimeInput.js';
import { emptyFilters, filterTasks, groupTasks, isOverdue, moveTask, taskSection, taskStatus } from '../node_modules/.cache/kairos-tests/taskModel.js';

const now = new Date(2026, 10, 21, 14, 30);
const task = (id, changes = {}) => ({ id, title: id, notes: '', priority: 'medium', manualOrder: 0, ...changes });

test('optional date and time pickers keep native inputs empty until a value is chosen', () => {
  for (const [type, value, emptyText] of [
    ['date', '2026-11-21', 'No date set'],
    ['time', '16:45', 'No time set'],
    ['datetime-local', '2026-11-21T16:45', 'Choose date and time'],
  ]) {
    const render = (value) => renderToStaticMarkup(createElement(OptionalDateTimeInput, { type, value, emptyText, onChange() {} }));
    const empty = render('');
    assert.ok(empty.includes('data-empty="true"'));
    assert.ok(empty.includes(`type="${type}" value=""`));
    assert.ok(empty.includes(emptyText));

    const filled = render(value);
    assert.ok(filled.includes('data-empty="false"'));
    assert.ok(filled.includes(`type="${type}" value="${value}"`));
    assert.ok(!filled.includes(emptyText));
  }
});

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

test('search uses titles and notes across active task sections', () => {
  const tasks = [
    task('today', { title: 'Ｐｌａｎ today', scheduledDate: '2026-11-21' }),
    task('upcoming', { notes: 'PLAN tomorrow', scheduledDate: '2026-11-22' }),
    task('no date', { notes: 'plan someday' }),
    task('completed', { title: 'Plan', completedAt: '2026-11-21T09:00:00Z' }),
    task('trashed', { title: 'Plan', trashedAt: '2026-11-21T09:00:00Z' }),
    task('skipped', { title: 'Plan', skippedAt: '2026-11-21T09:00:00Z' }),
    task('other'),
  ];
  const grouped = groupTasks(filterTasks(tasks, ' plan ', emptyFilters, now), 'automatic', now);
  assert.deepEqual(Object.values(grouped).map((items) => items.map(({ id }) => id)), [['today'], ['upcoming'], ['no date']]);
  assert.equal(filterTasks(tasks, ' ', emptyFilters, now).length, 4);
});

test('search and category, priority, schedule, due date and overdue filters all apply', () => {
  const matching = { notes: 'Homework', categoryId: 'study', priority: 'high', scheduledDate: '2026-11-21', dueDate: '2026-11-21', dueTime: '13:00' };
  const tasks = [
    task('matches', matching),
    task('query', { ...matching, notes: '' }),
    task('category', { ...matching, categoryId: 'home' }),
    task('priority', { ...matching, priority: 'low' }),
    task('schedule', { ...matching, scheduledDate: '2026-11-22' }),
    task('due', { ...matching, dueDate: '2026-11-20' }),
    task('overdue', { ...matching, dueTime: '15:00' }),
  ];
  const filters = { categoryId: 'study', priority: 'high', scheduledDate: '2026-11-21', dueDate: '2026-11-21', overdue: 'yes' };
  assert.deepEqual(filterTasks(tasks, 'homework', filters, now).map(({ id }) => id), ['matches']);
  assert.deepEqual(filterTasks(tasks, 'homework', { ...filters, overdue: 'no' }, now).map(({ id }) => id), ['overdue']);
  assert.equal(filterTasks(tasks, 'homework', emptyFilters, now).length, 6);
});
