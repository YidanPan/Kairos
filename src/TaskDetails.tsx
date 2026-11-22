import { useState } from 'react';
import { Modal } from './TaskModal';
import { ScopeOptions, TaskAdvanced } from './TaskAdvanced';
import { TaskReminders } from './ReminderControls';
import { localDay, priorities } from './taskModel';
import type { Category, Change, EditScope, ScheduleChange, Subtask, Task, TaskAction } from './taskModel';

export function CategoryOptions({ categories, empty }: { categories: Category[]; empty: string }) {
  return <><option value="">{empty}</option>{categories.map((category) => <option key={category.id} value={category.id}>{category.name}</option>)}</>;
}

function DateField({ label, value, change, disabled }: { label: string; value: string; change: (value: string) => void; disabled: boolean }) {
  return <div className="task-date-field">
    <label>{label}<input type="date" value={value} disabled={disabled} onChange={(event) => change(event.target.value)} /></label>
    <button type="button" disabled={disabled} aria-label={`Set ${label.toLowerCase()} to today`} onClick={() => change(localDay(new Date()))}>Today</button>
  </div>;
}

export function TaskDetails({ task, categories, history, subtasks, run, onClose }: {
  task: Task; categories: Category[]; history: ScheduleChange[]; subtasks: Subtask[];
  run: (action: TaskAction) => Promise<boolean>; onClose: () => void;
}) {
  const [title, setTitle] = useState(task.title);
  const [notes, setNotes] = useState(task.notes);
  const [due, setDue] = useState(task.dueDate ?? '');
  const [time, setTime] = useState(task.dueTime ?? '');
  const [failure, setFailure] = useState('');
  const [scope, setScope] = useState<EditScope>('this');
  const readOnly = Boolean(task.trashedAt || task.skippedAt);

  function save(change: Change) {
    setFailure('');
    void run(task.seriesId ? { kind: 'editOccurrence', id: task.id, change, scope } : { kind: 'edit', id: task.id, change })
      .then((saved) => { if (!saved) setFailure('This change could not be saved. Try again.'); });
  }

  function saveDeadline(date: string, time: string) {
    setDue(date);
    const nextTime = date ? time : '';
    setTime(nextTime);
    save({ field: 'deadline', value: { date: date || null, time: nextTime || null } });
  }

  return <Modal title="Task details" onClose={onClose}>
    <div className="task-form">
      {task.seriesId && !readOnly && <label>Apply task edits to<select value={scope} onChange={(event) => setScope(event.target.value as EditScope)}><ScopeOptions /></select></label>}
      {task.originSeriesId && !task.seriesId && <p className="task-hint">From a repeating task.</p>}
      <label>Task title<input value={title} readOnly={readOnly} onChange={(event) => setTitle(event.target.value)}
        onBlur={() => {
          if (readOnly) return;
          const next = title.trim();
          if (!next) { setTitle(task.title); setFailure('Enter a task title.'); }
          else if (next !== task.title) save({ field: 'title', value: next });
        }} onKeyDown={(event) => { if (event.key === 'Enter') event.currentTarget.blur(); }} /></label>
      <div className="task-form-pair">
        <label>Priority<select value={task.priority} disabled={readOnly} onChange={(event) => save({ field: 'priority', value: event.target.value as Task['priority'] })}>
          {priorities.map((priority) => <option key={priority} value={priority}>{priority[0].toUpperCase() + priority.slice(1)}</option>)}
        </select></label>
        <label>Category<select value={task.categoryId ?? ''} disabled={readOnly} onChange={(event) => save({ field: 'category', value: event.target.value || null })}>
          <CategoryOptions categories={categories} empty="None" />
        </select></label>
      </div>
      <DateField label="Scheduled date" value={task.scheduledDate ?? ''} disabled={readOnly} change={(value) => save({ field: 'scheduledDate', value: value || null })} />
      <DateField label="Due date" value={due} disabled={readOnly} change={(value) => saveDeadline(value, time)} />
      {due && <label>Due time<input type="time" value={time} disabled={readOnly} onChange={(event) => saveDeadline(due, event.target.value)} />{!time && <small>Defaults to 23:59</small>}</label>}
      <label>Notes<textarea rows={3} value={notes} readOnly={readOnly} onChange={(event) => setNotes(event.target.value)}
        onBlur={() => { if (!readOnly && notes !== task.notes) save({ field: 'notes', value: notes }); }} /></label>
      {failure && <p className="task-hint" role="alert">{failure}</p>}
      {history.length > 0 && <details className="task-history"><summary>Schedule history</summary><ul>
        {history.map((entry, index) => <li key={`${entry.changedAt}-${index}`}>{entry.previousDate} → {entry.nextDate ?? 'No Date'}{entry.automatic && ' · Rescheduled'}</li>)}
      </ul></details>}
      {!task.completedAt && !readOnly && <TaskReminders key={task.id} task={task} />}
      <TaskAdvanced task={task} subtasks={subtasks} run={run} onClose={onClose} />
    </div>
  </Modal>;
}
