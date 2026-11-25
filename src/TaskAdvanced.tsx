import { useState } from 'react';
import { OptionalDateTimeInput } from './OptionalDateTimeInput';
import type { EditScope, Frequency, RecurrenceRule, RepeatEdit, Subtask, Task, TaskAction } from './taskModel';

export type DraftSubtask = Pick<Subtask, 'id' | 'title'> & { completed: boolean };
export type TaskCommand = Extract<TaskAction, { kind: 'skipOccurrence' | 'trashTask' }>;

const weekdays = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
const defaultRule: RecurrenceRule = { frequency: 'days', interval: 1, weekdays: [], monthDay: null, endDate: null };
const oncePer = { days: 'Daily', weeks: 'Weekly', months: 'Monthly' };

export function ScopeOptions({ includeThis = true }: { includeThis?: boolean }) {
  return <>{includeThis && <option value="this">Only This Task</option>}
    <option value="future">This and Future Tasks</option><option value="entire">Entire Series</option></>;
}

function RepeatForm({ task, repeat, onChange }: { task: Task; repeat: RepeatEdit; onChange: (repeat: RepeatEdit) => void }) {
  const { rule, scope } = repeat;
  const setRule = (rule: RecurrenceRule) => onChange({ rule, scope });
  return <div className="task-form repeat-form">
    <label className="inline-check"><input type="checkbox" checked={Boolean(rule)} onChange={(event) => {
      onChange({ rule: event.target.checked ? task.recurrence ?? defaultRule : null, scope: scope === 'this' ? 'future' : scope });
    }} />Repeat task</label>
    {rule && <>
      {!task.scheduledDate && <p className="task-hint">Choose a scheduled date to start repeating.</p>}
      <div className="task-form-pair">
        <label>Repeat every<input type="number" min="1" max="65535" step="1" value={rule.interval || ''} required
          onChange={(event) => setRule({ ...rule, interval: Number(event.target.value) })} /></label>
        <label>Frequency<select value={rule.frequency} onChange={(event) => {
          const frequency = event.target.value as Frequency;
          setRule({ ...rule, frequency, monthDay: frequency === 'months' ? rule.monthDay : null });
        }}>
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
      <label>End date<OptionalDateTimeInput type="date" value={rule.endDate ?? ''} emptyText="Choose date"
        onChange={(value) => setRule({ ...rule, endDate: value || null })} />
        {!rule.endDate && <small>No end date</small>}</label>
    </>}
    {task.seriesId && <label>{rule ? 'Change repeat rule for' : 'Remove repeat from'}
      <select value={scope} onChange={(event) => onChange({ rule, scope: event.target.value as EditScope })}>
        <ScopeOptions includeThis={!rule} />
      </select></label>}
  </div>;
}

function SubtaskList({ inactive, items, onChange }: { inactive: boolean; items: DraftSubtask[]; onChange: (items: DraftSubtask[]) => void }) {
  const [title, setTitle] = useState('');
  const done = items.filter((item) => item.completed).length;

  return <details className="task-extras">
    <summary>Subtasks <span>{done} / {items.length}</span></summary>
    <ol className="subtask-list">
      {items.map((item, index) => <li key={item.id}>
        <input type="checkbox" checked={item.completed} disabled={inactive} aria-label={`Complete subtask ${item.title}`}
          onChange={(event) => onChange(items.map((other) => other.id === item.id ? { ...other, completed: event.target.checked } : other))} />
        <input aria-label={`Subtask ${index + 1} title`} value={item.title} readOnly={inactive}
          onChange={(event) => onChange(items.map((other) => other.id === item.id ? { ...other, title: event.target.value } : other))} />
        {!inactive && <div className="subtask-actions">
          {[-1, 1].map((offset) => <button key={offset} type="button" disabled={!items[index + offset]}
            aria-label={`Move subtask ${item.title} ${offset < 0 ? 'up' : 'down'}`}
            onClick={() => {
              const next = [...items];
              next[index] = items[index + offset];
              next[index + offset] = item;
              onChange(next);
            }}>{offset < 0 ? '↑' : '↓'}</button>)}
          <button type="button" aria-label={`Remove subtask ${item.title}`} onClick={() => onChange(items.filter((other) => other.id !== item.id))}>×</button>
        </div>}
      </li>)}
    </ol>
    {!inactive && <form className="subtask-add" onSubmit={(event) => {
      event.preventDefault();
      if (!title.trim()) return;
      onChange([...items, { id: crypto.randomUUID(), title: title.trim(), completed: false }]);
      setTitle('');
    }}>
      <input aria-label="New subtask title" placeholder="New subtask…" value={title} onChange={(event) => setTitle(event.target.value)} />
      <button type="submit" disabled={!title.trim()}>Add subtask</button>
    </form>}
  </details>;
}

export function TaskAdvanced({ task, subtasks, onSubtasksChange, repeat, onRepeatChange, onCommand }: {
  task: Task; subtasks: DraftSubtask[]; onSubtasksChange: (items: DraftSubtask[]) => void;
  repeat: RepeatEdit; onRepeatChange: (repeat: RepeatEdit) => void; onCommand: (command: TaskCommand) => void;
}) {
  const inactive = Boolean(task.completedAt || task.trashedAt || task.skippedAt);
  const { rule } = repeat;
  const pattern = rule && (rule.interval === 1 ? oncePer[rule.frequency] : `Every ${rule.interval} ${rule.frequency}`);

  return <>
    <SubtaskList inactive={inactive} items={subtasks} onChange={onSubtasksChange} />
    {!inactive && <details className="task-extras"><summary>Repeat{pattern && ` · ${pattern}`}</summary>
      <RepeatForm task={task} repeat={repeat} onChange={onRepeatChange} />
      {task.seriesId && <button className="task-skip" type="button" onClick={() => onCommand({ kind: 'skipOccurrence', id: task.id })}>Skip this occurrence…</button>}
    </details>}
    {!task.trashedAt && !task.skippedAt && <div className="task-actions">
      <button type="button" onClick={() => onCommand({ kind: 'trashTask', id: task.id, scope: 'this' })}>Move to Trash…</button>
    </div>}
  </>;
}
