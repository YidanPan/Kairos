import { useState } from 'react';
import { Modal } from './TaskDetails';
import { localDay } from './taskModel';
import type { Task, TaskAction } from './taskModel';

export type HistoryKind = 'completed' | 'trash' | 'skipped';
const headings = { completed: 'Completed', trash: 'Trash', skipped: 'Skipped' };
const timestamp = (task: Task, kind: HistoryKind) => kind === 'completed' ? task.completedAt : kind === 'trash' ? task.trashedAt : task.skippedAt;
const timeFormat = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' });

interface Confirmation { title: string; description: string; label: string; action: TaskAction }

export function TaskHistory({ kind, items, run, openTask }: { kind: HistoryKind; items: Task[]; run: (action: TaskAction) => Promise<boolean>; openTask: (id: string) => void }) {
  const [query, setQuery] = useState('');
  const [day, setDay] = useState('');
  const [selected, setSelected] = useState<string[]>([]);
  const [confirmation, setConfirmation] = useState<Confirmation>();
  const [busy, setBusy] = useState(false);
  const search = query.trim().toLocaleLowerCase();
  const visible = items.filter((task) => {
    const date = timestamp(task, kind);
    return (!search || [task.title, task.notes].some((value) => value.toLocaleLowerCase().includes(search)))
      && (!day || (date && localDay(new Date(date)) === day));
  }).sort((left, right) => (timestamp(right, kind) ?? '').localeCompare(timestamp(left, kind) ?? ''));
  const selectedVisible = visible.filter((task) => selected.includes(task.id)).map((task) => task.id);

  function toggle(id: string, checked: boolean) {
    setSelected((previous) => checked ? [...previous, id] : previous.filter((item) => item !== id));
  }

  async function submit(action: TaskAction) {
    setBusy(true);
    if (await run(action)) { setConfirmation(undefined); setSelected([]); }
    setBusy(false);
  }

  function purge(ids: string[] | null) {
    setConfirmation({
      title: 'Permanently delete tasks?',
      description: ids ? 'The selected tasks will be permanently removed.' : 'All tasks in Trash will be permanently removed.',
      label: ids ? 'Delete permanently' : 'Empty Trash', action: { kind: 'purgeTrash', ids },
    });
  }

  return <section className="task-section task-archive">
    <h3>{headings[kind]}</h3>
    <div className="archive-controls">
      <input type="search" aria-label={`Search ${headings[kind]}`} placeholder="Search titles or notes" value={query} onChange={(event) => setQuery(event.target.value)} />
      <input type="date" aria-label={`Browse ${headings[kind]} by date`} value={day} onChange={(event) => setDay(event.target.value)} />
    </div>
    {visible.length ? <ul>{visible.map((task) => <li className="archive-row" key={task.id}>
      <div className="archive-selection">
        {kind !== 'skipped' && <input type="checkbox" aria-label={`Select ${task.title}`} checked={selected.includes(task.id)} onChange={(event) => toggle(task.id, event.target.checked)} />}
        <button type="button" className="task-title" onClick={() => openTask(task.id)}>{task.title}</button>
      </div>
      <time>{timeFormat.format(new Date(timestamp(task, kind) ?? 0))}</time>
      {kind === 'completed' && <div className="task-actions">
        <button type="button" disabled={busy} aria-label={`Restore ${task.title}`} onClick={() => void submit({ kind: 'setCompleted', id: task.id, completed: false })}>Restore</button>
        <button type="button" disabled={busy} aria-label={`Move ${task.title} to Trash`} onClick={() => void submit({ kind: 'trashCompleted', ids: [task.id] })}>Move to Trash</button>
      </div>}
      {kind === 'trash' && <div className="task-actions">
        <button type="button" disabled={busy} aria-label={`Restore ${task.title}`} onClick={() => void submit({ kind: 'restoreTrash', ids: [task.id] })}>Restore</button>
        <button type="button" className="task-danger" disabled={busy} aria-label={`Permanently delete ${task.title}`} onClick={() => purge([task.id])}>Delete permanently</button>
      </div>}
    </li>)}</ul> : <p className="task-hint">{items.length ? 'No matching tasks.' : `No ${kind === 'trash' ? 'trashed' : kind} tasks.`}</p>}
    {items.length > 0 && kind !== 'skipped' && <div className="task-actions archive-actions">
      {kind === 'completed' && <>
        {selectedVisible.length > 0 && <button type="button" disabled={busy} onClick={() => void submit({ kind: 'trashCompleted', ids: selectedVisible })}>Move selected to Trash</button>}
        <button type="button" disabled={busy} onClick={() => setConfirmation({ title: 'Clear completed history?', description: 'All completed tasks will move to Trash.', label: 'Move all to Trash', action: { kind: 'trashCompleted', ids: null } })}>Clear history…</button>
      </>}
      {kind === 'trash' && <>
        {selectedVisible.length > 0 && <><button type="button" disabled={busy} onClick={() => void submit({ kind: 'restoreTrash', ids: selectedVisible })}>Restore selected</button>
          <button type="button" disabled={busy} className="task-danger" onClick={() => purge(selectedVisible)}>Delete selected…</button></>}
        <button type="button" disabled={busy} className="task-danger" onClick={() => purge(null)}>Empty Trash…</button>
      </>}
    </div>}
    {confirmation && <Modal title={confirmation.title} onClose={() => setConfirmation(undefined)}>
      <p className="task-hint">{confirmation.description}</p>
      <div className="task-actions"><button type="button" onClick={() => setConfirmation(undefined)}>Cancel</button>
        <button type="button" className="task-danger" disabled={busy} onClick={() => void submit(confirmation.action)}>{confirmation.label}</button></div>
    </Modal>}
  </section>;
}
