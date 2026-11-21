import { useState } from 'react';
import type { EditScope, Frequency, RecurrenceRule, Subtask, Task, TaskAction } from './taskModel';

const weekdays = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
const defaultRule: RecurrenceRule = { frequency: 'days', interval: 1, weekdays: [], monthDay: null, endDate: null };
const oncePer = { days: 'Daily', weeks: 'Weekly', months: 'Monthly' };

export function ScopeOptions({ includeThis = true }: { includeThis?: boolean }) {
  return <>{includeThis && <option value="this">Only This Task</option>}
    <option value="future">This and Future Tasks</option><option value="entire">Entire Series</option></>;
}

function RepeatForm({ task, save }: { task: Task; save: (action: TaskAction) => Promise<boolean> }) {
  const [rule, setRule] = useState(task.recurrence ?? defaultRule);
  const [scope, setScope] = useState<EditScope>('future');
  const [removalScope, setRemovalScope] = useState<EditScope>('future');
  const [busy, setBusy] = useState(false);
  const missingSchedule = !task.scheduledDate;

  async function submit(value: RecurrenceRule | null) {
    setBusy(true);
    await save({ kind: 'saveRecurrence', id: task.id, rule: value, scope: value ? scope : removalScope });
    setBusy(false);
  }

  return <form className="task-form repeat-form" onSubmit={(event) => { event.preventDefault(); void submit(rule); }}>
    {missingSchedule && <p className="task-hint">Choose a scheduled date to start repeating.</p>}
    <div className="task-form-pair">
      <label>Repeat every<input type="number" min="1" max="65535" step="1" value={rule.interval || ''} required
        onChange={(event) => setRule({ ...rule, interval: Number(event.target.value) })} /></label>
      <label>Frequency<select value={rule.frequency} onChange={(event) => setRule({ ...rule, frequency: event.target.value as Frequency })}>
        <option value="days">Days</option><option value="weeks">Weeks</option><option value="months">Months</option>
      </select></label>
    </div>
    {rule.frequency === 'weeks' && <fieldset className="weekday-options"><legend>Repeat on</legend>
      {weekdays.map((day, index) => <label key={day}><input type="checkbox" checked={rule.weekdays.includes(index)}
        onChange={(event) => setRule({ ...rule, weekdays: event.target.checked ? [...rule.weekdays, index] : rule.weekdays.filter((day) => day !== index) })} />{day}</label>)}
      {!rule.weekdays.length && <p className="task-hint">Uses the scheduled weekday.</p>}
    </fieldset>}
    {rule.frequency === 'months' && <label>Day of month<input type="number" min="1" max="31" step="1" value={rule.monthDay ?? ''} placeholder="Scheduled day"
      onChange={(event) => setRule({ ...rule, monthDay: event.target.value ? Number(event.target.value) : null })} />
      <small>Months without this day are skipped.</small></label>}
    <label>End date<input type="date" value={rule.endDate ?? ''} onChange={(event) => setRule({ ...rule, endDate: event.target.value || null })} />
      {!rule.endDate && <small>No end date</small>}</label>
    {task.seriesId && <label>Change repeat rule for<select value={scope} onChange={(event) => setScope(event.target.value as EditScope)}><ScopeOptions includeThis={false} /></select></label>}
    <div className="task-actions"><button type="submit" className="task-primary" disabled={busy || missingSchedule}>Save repeat rule</button></div>
    {task.seriesId && <div className="repeat-removal">
      <label>Remove repeat from<select value={removalScope} onChange={(event) => setRemovalScope(event.target.value as EditScope)}><ScopeOptions /></select></label>
      <button type="button" disabled={busy} onClick={() => void submit(null)}>Remove repeat</button>
    </div>}
  </form>;
}

function SubtaskList({ task, items, save }: { task: Task; items: Subtask[]; save: (action: TaskAction) => Promise<boolean> }) {
  const [title, setTitle] = useState('');
  const [adding, setAdding] = useState(false);
  const inactive = Boolean(task.completedAt || task.trashedAt || task.skippedAt);
  const done = items.filter((item) => item.completedAt).length;

  return <details className="task-extras">
    <summary>Subtasks <span>{done} / {items.length}</span></summary>
    <ol className="subtask-list">
      {items.map((item, index) => <li key={item.id}>
        <input type="checkbox" checked={Boolean(item.completedAt)} disabled={inactive} aria-label={`Complete subtask ${item.title}`}
          onChange={(event) => void save({ kind: 'setSubtaskCompleted', taskId: task.id, id: item.id, completed: event.target.checked })} />
        <input aria-label={`Subtask title ${item.title}`} defaultValue={item.title} readOnly={inactive} onBlur={(event) => {
          const value = event.currentTarget.value.trim();
          if (!value) { event.currentTarget.value = item.title; return; }
          if (!inactive && value !== item.title) void save({ kind: 'saveSubtask', taskId: task.id, id: item.id, title: value });
        }} onKeyDown={(event) => { if (event.key === 'Enter') event.currentTarget.blur(); }} />
        {!inactive && <div className="subtask-actions">
          {[-1, 1].map((offset) => <button key={offset} type="button" disabled={!items[index + offset]}
            aria-label={`Move subtask ${item.title} ${offset < 0 ? 'up' : 'down'}`}
            onClick={() => void save({ kind: 'moveSubtask', taskId: task.id, id: item.id, neighborId: items[index + offset].id })}>{offset < 0 ? '↑' : '↓'}</button>)}
          <button type="button" aria-label={`Remove subtask ${item.title}`} onClick={() => void save({ kind: 'removeSubtask', taskId: task.id, id: item.id })}>×</button>
        </div>}
      </li>)}
    </ol>
    {!inactive && <form className="subtask-add" onSubmit={(event) => {
      event.preventDefault();
      if (!title.trim() || adding) return;
      setAdding(true);
      void save({ kind: 'saveSubtask', taskId: task.id, id: crypto.randomUUID(), title }).then((saved) => {
        if (saved) setTitle('');
        setAdding(false);
      });
    }}>
      <input aria-label="New subtask title" placeholder="New subtask…" value={title} onChange={(event) => setTitle(event.target.value)} />
      <button type="submit" disabled={adding || !title.trim()}>Add subtask</button>
    </form>}
  </details>;
}

export function TaskAdvanced({ task, subtasks, run, onClose }: {
  task: Task; subtasks: Subtask[]; run: (action: TaskAction) => Promise<boolean>; onClose: () => void;
}) {
  const [error, setError] = useState('');
  const [confirmTrash, setConfirmTrash] = useState(false);
  const [deleteScope, setDeleteScope] = useState<EditScope>('this');
  const inactive = Boolean(task.completedAt || task.trashedAt || task.skippedAt);
  const pattern = task.recurrence && (task.recurrence.interval === 1
    ? oncePer[task.recurrence.frequency] : `Every ${task.recurrence.interval} ${task.recurrence.frequency}`);

  async function save(action: TaskAction, close = false) {
    setError('');
    const saved = await run(action);
    if (!saved) setError('This change could not be saved. Check the details and try again.');
    if (saved && close) onClose();
    return saved;
  }

  return <>
    <SubtaskList task={task} items={subtasks} save={save} />
    {!inactive && <details className="task-extras"><summary>Repeat{pattern && ` · ${pattern}`}</summary>
      <RepeatForm key={JSON.stringify(task.recurrence)} task={task} save={save} />
      {task.seriesId && <button className="task-skip" type="button" onClick={() => void save({ kind: 'skipOccurrence', id: task.id }, true)}>Skip this occurrence</button>}
    </details>}
    {!task.trashedAt && !task.skippedAt && <div className="task-actions">
      {confirmTrash ? <>
        {task.seriesId && <select aria-label="Move to Trash scope" value={deleteScope} onChange={(event) => setDeleteScope(event.target.value as EditScope)}><ScopeOptions /></select>}
        <button type="button" onClick={() => setConfirmTrash(false)}>Cancel</button>
        <button type="button" className="task-danger" onClick={() => void save({ kind: 'trashTask', id: task.id, scope: deleteScope }, true)}>Move to Trash</button>
      </> : <button type="button" onClick={() => setConfirmTrash(true)}>Move to Trash…</button>}
    </div>}
    {error && <p className="task-hint" role="alert">{error}</p>}
  </>;
}
