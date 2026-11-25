import { useState } from 'react';
import { OptionalDateTimeInput } from './OptionalDateTimeInput';
import { Modal } from './TaskModal';
import { ScopeOptions, TaskAdvanced } from './TaskAdvanced';
import type { DraftSubtask, TaskCommand } from './TaskAdvanced';
import { TaskReminders } from './ReminderControls';
import { localDay, priorities } from './taskModel';
import type { Category, Change, EditScope, RepeatEdit, ScheduleChange, Subtask, SubtaskAction, Task, TaskAction, TaskReminderPreferences } from './taskModel';

export function CategoryOptions({ categories, empty }: { categories: Category[]; empty: string }) {
  return <><option value="">{empty}</option>{categories.map((category) => <option key={category.id} value={category.id}>{category.name}</option>)}</>;
}

function DateField({ label, value, change, disabled }: { label: string; value: string; change: (value: string) => void; disabled: boolean }) {
  return <div className="task-date-field">
    <label>{label}<OptionalDateTimeInput type="date" value={value} emptyText="No date set" disabled={disabled} onChange={change} /></label>
    <button type="button" disabled={disabled} aria-label={`Set ${label.toLowerCase()} to today`} onClick={() => change(localDay(new Date()))}>Today</button>
  </div>;
}

function subtaskChanges(taskId: string, original: Subtask[], draft: DraftSubtask[]): SubtaskAction[] {
  const previous = new Map(original.map((item) => [item.id, item]));
  const retained = new Set(draft.map((item) => item.id));
  const order = original.filter((item) => retained.has(item.id)).map((item) => item.id);
  const changes: SubtaskAction[] = original.filter((item) => !retained.has(item.id))
    .map(({ id }) => ({ kind: 'removeSubtask', taskId, id }));

  for (const { id, title, completed } of draft) {
    const before = previous.get(id);
    if (!before) order.push(id);
    if (!before || title.trim() !== before.title) changes.push({ kind: 'saveSubtask', taskId, id, title });
    if (Boolean(before?.completedAt) !== completed) changes.push({ kind: 'setSubtaskCompleted', taskId, id, completed });
  }
  for (const [index, { id }] of draft.entries()) {
    const neighborId = order[index];
    if (id === neighborId) continue;
    const source = order.indexOf(id, index + 1);
    order[source] = neighborId;
    order[index] = id;
    changes.push({ kind: 'moveSubtask', taskId, id, neighborId });
  }
  return changes;
}

export function TaskDetails({ task, categories, history, subtasks, run, onClose }: {
  task: Task; categories: Category[]; history: ScheduleChange[]; subtasks: Subtask[];
  run: (action: TaskAction, onError: (message: string) => void) => Promise<boolean>; onClose: () => void;
}) {
  const [original] = useState(task);
  const [draft, setDraft] = useState(task);
  const [originalSubtasks] = useState(subtasks);
  const [draftSubtasks, setDraftSubtasks] = useState<DraftSubtask[]>(() => subtasks.map(({ id, title, completedAt }) => ({ id, title, completed: Boolean(completedAt) })));
  const [repeat, setRepeat] = useState<RepeatEdit>({ rule: task.recurrence, scope: 'future' });
  const [reminders, setReminders] = useState<TaskReminderPreferences>();
  const [command, setCommand] = useState<TaskCommand>();
  const [saving, setSaving] = useState(false);
  const [failure, setFailure] = useState('');
  const [scope, setScope] = useState<EditScope>('this');
  const readOnly = Boolean(task.trashedAt || task.skippedAt);

  async function confirm() {
    setFailure('');
    let action: TaskAction;
    if (command) {
      action = command;
    } else {
      const title = draft.title.trim();
      if (!title) { setFailure('Enter a task title.'); return; }
      const repeatEdit = JSON.stringify(repeat.rule) === JSON.stringify(original.recurrence) ? undefined : repeat;
      if (repeat.rule && !draft.scheduledDate && !task.completedAt) {
        setFailure('Choose a scheduled date for the repeating task.'); return;
      }
      if (repeatEdit?.rule) {
        const { interval, frequency, monthDay } = repeatEdit.rule;
        if (!Number.isInteger(interval) || interval < 1 || interval > 65535) {
          setFailure('Enter a repeat interval from 1 to 65535.'); return;
        }
        if (frequency === 'months' && monthDay !== null && (!Number.isInteger(monthDay) || monthDay < 1 || monthDay > 31)) {
          setFailure('Enter a day from 1 to 31.'); return;
        }
      }

      const changes: Change[] = [];
      if (title !== original.title) changes.push({ field: 'title', value: title });
      if (draft.priority !== original.priority) changes.push({ field: 'priority', value: draft.priority });
      if (draft.categoryId !== original.categoryId) changes.push({ field: 'category', value: draft.categoryId });
      if (draft.scheduledDate !== original.scheduledDate) changes.push({ field: 'scheduledDate', value: draft.scheduledDate });
      if (draft.dueDate !== original.dueDate || draft.dueTime !== original.dueTime) {
        changes.push({ field: 'deadline', value: { date: draft.dueDate, time: draft.dueTime } });
      }
      if (draft.notes !== original.notes) changes.push({ field: 'notes', value: draft.notes });
      const editedSubtasks = subtaskChanges(task.id, originalSubtasks, draftSubtasks);
      if (!changes.length && !repeatEdit && !reminders && !editedSubtasks.length) { onClose(); return; }
      action = { kind: 'confirmTaskEdits', id: task.id, scope, changes, repeat: repeatEdit, reminders, subtasks: editedSubtasks };
    }
    setSaving(true);
    const saved = await run(action, setFailure);
    if (saved) onClose();
    else setSaving(false);
  }

  function chooseCommand(next?: TaskCommand) {
    setFailure('');
    setCommand(next);
  }

  return <Modal title="Task details" onClose={() => { if (!saving) onClose(); }}>
    <fieldset className="task-form task-detail-fields" hidden={Boolean(command)} disabled={saving}>
      {task.seriesId && !readOnly && <label>Apply task edits to<select value={scope} onChange={(event) => setScope(event.target.value as EditScope)}><ScopeOptions /></select></label>}
      {task.originSeriesId && !task.seriesId && <p className="task-hint">From a repeating task.</p>}
      <label>Task title<input value={draft.title} readOnly={readOnly} onChange={(event) => setDraft({ ...draft, title: event.target.value })} /></label>
      <div className="task-form-pair">
        <label>Priority<select value={draft.priority} disabled={readOnly} onChange={(event) => setDraft({ ...draft, priority: event.target.value as Task['priority'] })}>
          {priorities.map((priority) => <option key={priority} value={priority}>{priority[0].toUpperCase() + priority.slice(1)}</option>)}
        </select></label>
        <label>Category<select value={draft.categoryId ?? ''} disabled={readOnly} onChange={(event) => setDraft({ ...draft, categoryId: event.target.value || null })}>
          <CategoryOptions categories={categories} empty="None" />
        </select></label>
      </div>
      <DateField label="Scheduled date" value={draft.scheduledDate ?? ''} disabled={readOnly} change={(value) => setDraft({ ...draft, scheduledDate: value || null })} />
      <DateField label="Due date" value={draft.dueDate ?? ''} disabled={readOnly}
        change={(value) => setDraft({ ...draft, dueDate: value || null, dueTime: value ? draft.dueTime : null })} />
      {draft.dueDate && <label>Due time
        <OptionalDateTimeInput type="time" value={draft.dueTime ?? ''} emptyText="No time set" disabled={readOnly}
          onChange={(value) => setDraft({ ...draft, dueTime: value || null })} />
        {!draft.dueTime && <small>Defaults to 23:59</small>}
      </label>}
      <label>Notes<textarea rows={3} value={draft.notes} readOnly={readOnly} onChange={(event) => setDraft({ ...draft, notes: event.target.value })} /></label>
      {history.length > 0 && <details className="task-history"><summary>Schedule history</summary><ul>
        {history.map((entry, index) => <li key={`${entry.changedAt}-${index}`}>{entry.previousDate} → {entry.nextDate ?? 'No Date'}{entry.automatic && ' · Rescheduled'}</li>)}
      </ul></details>}
      {!task.completedAt && !readOnly && <TaskReminders task={draft} draft={reminders} onChange={setReminders} />}
      <TaskAdvanced task={{ ...task, scheduledDate: draft.scheduledDate }} subtasks={draftSubtasks} onSubtasksChange={setDraftSubtasks}
        repeat={repeat} onRepeatChange={setRepeat} onCommand={chooseCommand} />
    </fieldset>
    {command && <div className="task-form">
      <h3>{command.kind === 'trashTask' ? 'Move to Trash?' : 'Skip this occurrence?'}</h3>
      {task.seriesId && command.kind === 'trashTask' && <label>Move to Trash scope
        <select value={command.scope} disabled={saving} onChange={(event) => setCommand({ ...command, scope: event.target.value as EditScope })}><ScopeOptions /></select>
      </label>}
    </div>}
    {!readOnly && <footer className="task-detail-footer">
      {failure && <p className="task-hint" role="alert">{failure}</p>}
      <div className="task-actions">
        <button type="button" disabled={saving} onClick={() => command ? chooseCommand() : onClose()}>Cancel</button>
        <button type="button" className="task-primary" disabled={saving} onClick={() => void confirm()}>{saving ? 'Saving…' : 'Confirm'}</button>
      </div>
    </footer>}
  </Modal>;
}
