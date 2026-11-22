import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Modal } from './TaskModal';
import type { Task } from './taskModel';

type Quiet = 'off' | 'silent' | 'suppress';
type Sound = 'system' | 'silent' | 'custom';
type Rule = { kind: 'offset'; minutes: number } | { kind: 'custom'; at: string };
type Period = { days: number[]; start: string; end: string; mode: Quiet };
type Preferences = { manualDnd: Quiet; periods: Period[]; sound: Sound };
type TaskPreferences = { overrides: boolean; allowDuringDnd: boolean; rules: Rule[] };
type Notice = { id: number; taskId: string; title: string; deadline: string | null; createdAt: number; firstDueAt: number; count: number; nativeStatus: 'pending' | 'accepted' | 'unavailable' };
type State = { preferences: Preferences; task: TaskPreferences; categoryOffsets: number[]; notices: Notice[]; hasCustomSound: boolean };
type NotificationAction = { kind: 'open'; id: string } | { kind: 'snooze'; id: number };
type Action =
  | { kind: 'savePreferences'; preferences: Preferences }
  | { kind: 'saveTask'; id: string; preferences: TaskPreferences }
  | { kind: 'saveCategory'; id: string; offsets: number[] }
  | { kind: 'importSound'; bytes: number[] }
  | { kind: 'previewSound' }
  | { kind: 'dismiss'; id: number }
  | { kind: 'snooze'; id: number; minutes?: number; until?: string };

const offsets = [[0, 'At deadline'], [10, '10 minutes before'], [30, '30 minutes before'], [60, '1 hour before'], [1440, '1 day before']] as const;
const weekdays = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
const errorText = (value: unknown) => typeof value === 'string' ? value : 'Could not load or save reminders.';
const request = (action?: Action, taskId?: string, categoryId?: string) =>
  invoke<State>('reminder_state', { action: action ?? null, taskId: taskId ?? null, categoryId: categoryId ?? null });

function QuietOptions({ off = true }: { off?: boolean }) {
  return <>{off && <option value="off">Off</option>}<option value="silent">Silent only</option><option value="suppress">Suppress notifications</option></>;
}

function OffsetChoices({ values, onChange }: { values: number[]; onChange: (values: number[]) => void }) {
  return <div className="reminder-choices">{offsets.map(([value, label]) => <label key={value}><input type="checkbox" checked={values.includes(value)}
    onChange={(event) => onChange(event.target.checked ? [...values, value] : values.filter((item) => item !== value))} />{label}</label>)}</div>;
}

function TaskRules({ task, initial, inherited }: { task: Task; initial: TaskPreferences; inherited: number[] }) {
  const [draft, setDraft] = useState(initial);
  const [custom, setCustom] = useState('');
  const [status, setStatus] = useState('');
  const [busy, setBusy] = useState(false);
  const customRules = draft.rules.filter((rule) => rule.kind === 'custom');
  const taskOffsets = draft.rules.flatMap((rule) => rule.kind === 'offset' ? [rule.minutes] : []);

  async function save() {
    setBusy(true);
    setStatus('');
    try { await request({ kind: 'saveTask', id: task.id, preferences: draft }, task.id); setStatus('Saved.'); }
    catch (error) { setStatus(errorText(error)); }
    finally { setBusy(false); }
  }

  return <div className="task-form reminder-form">
    <label>Reminder source<select value={draft.overrides ? 'custom' : 'category'} onChange={(event) => setDraft({ ...draft, overrides: event.target.value === 'custom' })}>
      <option value="category">Use category defaults</option><option value="custom">Set task reminders</option>
    </select></label>
    {!draft.overrides && <p className="task-hint">{inherited.length ? offsets.filter(([value]) => inherited.includes(value)).map(([, name]) => name).join(' · ') : 'No inherited reminders.'}</p>}
    {draft.overrides && <>
      <OffsetChoices values={taskOffsets} onChange={(values) => setDraft({ ...draft, rules: [...customRules, ...values.map((minutes): Rule => ({ kind: 'offset', minutes }))] })} />
      {!task.dueDate && taskOffsets.length > 0 && <p className="task-hint">Deadline reminders wait for a due date.</p>}
      {customRules.map(({ at }) => <div className="task-actions" key={at}><time>{at.replace('T', ' ')}</time>
        <button type="button" aria-label={'Remove reminder ' + at} onClick={() => setDraft({ ...draft, rules: draft.rules.filter((rule) => rule.kind !== 'custom' || rule.at !== at) })}>Remove</button></div>)}
      <label>Custom reminder<input type="datetime-local" value={custom} onChange={(event) => setCustom(event.target.value)} /></label>
      <button type="button" disabled={!custom || customRules.some((rule) => rule.at === custom)} onClick={() => {
        setDraft({ ...draft, rules: [...draft.rules, { kind: 'custom', at: custom }] });
        setCustom('');
      }}>Add custom reminder</button>
    </>}
    <label className="inline-check"><input type="checkbox" checked={draft.allowDuringDnd}
      onChange={(event) => setDraft({ ...draft, allowDuringDnd: event.target.checked })} />Allow During DND</label>
    <button type="button" disabled={busy} onClick={() => void save()}>Save reminders</button>
    {status && <p className="task-hint" role="status">{status}</p>}
  </div>;
}

export function TaskReminders({ task }: { task: Task }) {
  const [state, setState] = useState<State>();
  const [problem, setProblem] = useState('');
  const [open, setOpen] = useState(false);
  useEffect(() => {
    if (!open) return;
    let active = true;
    void request(undefined, task.id, task.categoryId ?? undefined).then((value) => { if (active) setState(value); },
      (error: unknown) => { if (active) setProblem(errorText(error)); });
    return () => { active = false; };
  }, [open, task.id, task.categoryId]);

  return <details className="task-extras" onToggle={(event) => setOpen(event.currentTarget.open)}><summary>Reminders</summary>
    {open && (state ? <TaskRules task={task} initial={state.task} inherited={state.categoryOffsets} /> : <p role="status">{problem || 'Loading reminders…'}</p>)}
  </details>;
}

export function CategoryReminders({ id }: { id: string }) {
  const [values, setValues] = useState<number[]>();
  const [status, setStatus] = useState('');
  useEffect(() => {
    let active = true;
    void request(undefined, undefined, id).then((data) => { if (active) setValues(data.categoryOffsets); },
      (error: unknown) => { if (active) setStatus(errorText(error)); });
    return () => { active = false; };
  }, [id]);

  return <details className="task-extras"><summary>Default reminders</summary>
    {values && <><OffsetChoices values={values} onChange={setValues} /><button type="button" onClick={() => {
      void request({ kind: 'saveCategory', id, offsets: values }).then(() => setStatus('Saved.'), (error: unknown) => setStatus(errorText(error)));
    }}>Save defaults</button></>}
    {status && <p className="task-hint" role="status">{status}</p>}
  </details>;
}

function Settings({ data, save }: { data: State; save: (action: Action) => Promise<boolean> }) {
  const [preferences, setPreferences] = useState(data.preferences);
  const [status, setStatus] = useState('');
  const changePeriod = (index: number, period: Period) => setPreferences({ ...preferences, periods: preferences.periods.map((value, slot) => slot === index ? period : value) });

  return <details className="task-extras"><summary>Notification Settings</summary><div className="task-form">
    <label>Do Not Disturb<select value={preferences.manualDnd} onChange={(event) => setPreferences({ ...preferences, manualDnd: event.target.value as Quiet })}><QuietOptions /></select></label>
    <label>Sound<select value={preferences.sound} onChange={(event) => setPreferences({ ...preferences, sound: event.target.value as Sound })}>
      <option value="system">System Default</option><option value="silent">Silent</option><option value="custom">Custom WAV</option>
    </select></label>
    {preferences.sound === 'custom' && <>
      <label>Choose WAV file<input type="file" accept=".wav,audio/wav" onChange={(event) => {
        const file = event.target.files?.[0];
        if (!file) return;
        if (file.size > 12_000_000) { setStatus('Choose a WAV file smaller than 12 MB.'); return; }
        void file.arrayBuffer().then((buffer) => save({ kind: 'importSound', bytes: Array.from(new Uint8Array(buffer)) }))
          .then((saved) => { if (saved) setStatus('Sound file saved.'); }).catch((error: unknown) => setStatus(errorText(error)));
      }} /></label>
      <button type="button" disabled={!data.hasCustomSound} onClick={() => void save({ kind: 'previewSound' })}>Preview custom sound</button>
    </>}
    <p className="task-hint">Windows controls notification volume.</p>
    <h3>Quiet periods</h3>
    {preferences.periods.map((period, index) => <fieldset className="quiet-period" key={index}>
      <div className="task-form-pair">
        <label>Start<input type="time" value={period.start} onChange={(event) => changePeriod(index, { ...period, start: event.target.value })} /></label>
        <label>End<input type="time" value={period.end} onChange={(event) => changePeriod(index, { ...period, end: event.target.value })} /></label>
      </div>
      <div className="reminder-choices">{weekdays.map((label, day) => <label key={label}><input type="checkbox" checked={period.days.includes(day)}
        onChange={(event) => changePeriod(index, { ...period, days: event.target.checked ? [...period.days, day] : period.days.filter((value) => value !== day) })} />{label}</label>)}</div>
      {period.days.length === 0 && <p className="task-hint">Every day</p>}
      <label>Quiet mode<select value={period.mode} onChange={(event) => changePeriod(index, { ...period, mode: event.target.value as Quiet })}><QuietOptions off={false} /></select></label>
      <button type="button" onClick={() => setPreferences({ ...preferences, periods: preferences.periods.filter((_, slot) => slot !== index) })}>Remove period</button>
    </fieldset>)}
    <button type="button" onClick={() => setPreferences({ ...preferences, periods: [...preferences.periods, { days: [], start: '22:00', end: '07:00', mode: 'suppress' }] })}>Add quiet period</button>
    <button type="button" onClick={() => void save({ kind: 'savePreferences', preferences }).then((saved) => { if (saved) setStatus('Saved.'); })}>Save notification settings</button>
    {status && <p className="task-hint" role="status">{status}</p>}
  </div></details>;
}

export function ReminderHub({ openTask, completeTask }: { openTask: (id: string) => void; completeTask: (id: string) => Promise<boolean> }) {
  const [data, setData] = useState<State>();
  const [open, setOpen] = useState(false);
  const [problem, setProblem] = useState('');
  const [snoozing, setSnoozing] = useState<number>();
  const [until, setUntil] = useState('');

  useEffect(() => {
    let active = true;
    const refresh = () => {
      void request().then((data) => { if (active) setData(data); }, (error: unknown) => { if (active) setProblem(errorText(error)); });
    };
    refresh();
    const updates = listen('kairos-reminders', refresh);
    const receiveActions = () => {
      if (!active) return;
      void invoke<NotificationAction[]>('pending_notification_actions').then((actions) => {
        if (!active) return;
        for (const action of actions) {
          if (action.kind === 'open') {
            setOpen(false);
            setSnoozing(undefined);
            openTask(action.id);
          } else {
            refresh();
            setSnoozing(action.id);
            setUntil('');
            setOpen(true);
          }
        }
      }, (error: unknown) => { if (active) setProblem(errorText(error)); });
    };
    const activation = listen('kairos-notification-action', receiveActions);
    void activation.then(receiveActions, () => undefined);
    return () => { active = false; for (const listener of [updates, activation]) void listener.then((unlisten) => unlisten(), () => undefined); };
  }, [openTask]);

  async function save(action: Action) {
    setProblem('');
    try { setData(await request(action)); return true; }
    catch (error) { setProblem(errorText(error)); return false; }
  }

  const notices = data?.notices ?? [];
  const selected = notices.find((notice) => notice.id === snoozing);
  const orderedNotices = selected ? [selected, ...notices.filter((notice) => notice.id !== selected.id)] : notices;
  const count = notices.length;
  return <>
    <button type="button" onClick={() => {
      setOpen(true);
      void request().then(setData, (error: unknown) => setProblem(errorText(error)));
    }}>Reminders{count > 0 ? ' · ' + count : ''}</button>
    {open && <Modal title="Reminders" onClose={() => { setOpen(false); setSnoozing(undefined); }}>
      {problem && <p role="alert">{problem}</p>}
      {data ? <>
        {data.notices.length === 0 && <p className="task-hint">No pending reminders.</p>}
        {orderedNotices.map((notice) => <section key={notice.id} className="reminder-notice">
          <h3>{notice.title}</h3>
          {notice.deadline && <p className="task-hint">Due {notice.deadline}</p>}
          <p className="task-hint">{notice.count > 1 ? notice.count + ' reminders due' : 'Task reminder'}</p>
          {notice.nativeStatus === 'unavailable' && <p className="task-hint">Windows toast unavailable.</p>}
          <div className="task-actions">
            <button type="button" onClick={() => { setOpen(false); openTask(notice.taskId); }}>Open Task</button>
            <button type="button" onClick={() => void completeTask(notice.taskId).then((done) => { if (done) void save({ kind: 'dismiss', id: notice.id }); })}>Complete</button>
            <button type="button" onClick={() => { setSnoozing(notice.id); setUntil(''); }}>Snooze</button>
            <button type="button" onClick={() => void save({ kind: 'dismiss', id: notice.id })}>Dismiss</button>
          </div>
          {snoozing === notice.id && <div className="task-form">
            <div className="task-actions">{[5, 15, 30, 60].map((minutes) => <button key={minutes} type="button" onClick={() => void save({ kind: 'snooze', id: notice.id, minutes })}>{minutes} min</button>)}</div>
            <label>Custom snooze<input type="datetime-local" value={until} onChange={(event) => setUntil(event.target.value)} /></label>
            <button type="button" disabled={!until} onClick={() => void save({ kind: 'snooze', id: notice.id, until })}>Snooze until this time</button>
          </div>}
        </section>)}
        <Settings data={data} save={save} />
      </> : <p role="status">Loading reminders…</p>}
    </Modal>}
  </>;
}
