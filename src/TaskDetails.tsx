import { useEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { localDay, priorities } from './taskModel';
import type { Category, Change, ScheduleChange, Task } from './taskModel';

export function Modal({ title, children, onClose }: { title: string; children: ReactNode; onClose: () => void }) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => { ref.current?.showModal(); }, []);

  function close() {
    if (document.activeElement instanceof HTMLElement && ref.current?.contains(document.activeElement)) {
      document.activeElement.blur();
    }
    onClose();
  }

  return <dialog ref={ref} className="task-modal" aria-label={title}
    onCancel={(event) => { event.preventDefault(); close(); }}
    onClick={(event) => { if (event.target === ref.current) close(); }}>
    <div className="task-modal-content">
      <header className="task-modal-header"><h2>{title}</h2><button type="button" onClick={close} aria-label="Close dialog">×</button></header>
      {children}
    </div>
  </dialog>;
}

export function CategoryOptions({ categories, empty }: { categories: Category[]; empty: string }) {
  return <><option value="">{empty}</option>{categories.map((category) => <option key={category.id} value={category.id}>{category.name}</option>)}</>;
}

function DateField({ label, value, change }: { label: string; value: string; change: (value: string) => void }) {
  return <div className="task-date-field">
    <label>{label}<input type="date" value={value} onChange={(event) => change(event.target.value)} /></label>
    <button type="button" aria-label={`Set ${label.toLowerCase()} to today`} onClick={() => change(localDay(new Date()))}>Today</button>
  </div>;
}

export function TaskDetails({ task, categories, history, onChange, onClose }: {
  task: Task; categories: Category[]; history: ScheduleChange[];
  onChange: (change: Change) => Promise<boolean>; onClose: () => void;
}) {
  const [title, setTitle] = useState(task.title);
  const [notes, setNotes] = useState(task.notes);
  const [due, setDue] = useState(task.dueDate ?? '');
  const [time, setTime] = useState(task.dueTime ?? '');
  const [failure, setFailure] = useState('');

  function save(change: Change) {
    setFailure('');
    void onChange(change).then((saved) => { if (!saved) setFailure('This change could not be saved. Try again.'); });
  }

  function saveDeadline(date: string, time: string) {
    setDue(date);
    const nextTime = date ? time : '';
    setTime(nextTime);
    save({ field: 'deadline', value: { date: date || null, time: nextTime || null } });
  }

  return <Modal title="Task details" onClose={onClose}>
    <div className="task-form">
      <label>Task title<input value={title} onChange={(event) => setTitle(event.target.value)}
        onBlur={() => {
          const next = title.trim();
          if (!next) { setTitle(task.title); setFailure('Enter a task title.'); }
          else if (next !== task.title) save({ field: 'title', value: next });
        }} onKeyDown={(event) => { if (event.key === 'Enter') event.currentTarget.blur(); }} /></label>
      <div className="task-form-pair">
        <label>Priority<select value={task.priority} onChange={(event) => save({ field: 'priority', value: event.target.value as Task['priority'] })}>
          {priorities.map((priority) => <option key={priority} value={priority}>{priority[0].toUpperCase() + priority.slice(1)}</option>)}
        </select></label>
        <label>Category<select value={task.categoryId ?? ''} onChange={(event) => save({ field: 'category', value: event.target.value || null })}>
          <CategoryOptions categories={categories} empty="None" />
        </select></label>
      </div>
      <DateField label="Scheduled date" value={task.scheduledDate ?? ''} change={(value) => save({ field: 'scheduledDate', value: value || null })} />
      <DateField label="Due date" value={due} change={(value) => saveDeadline(value, time)} />
      {due && <label>Due time<input type="time" value={time} onChange={(event) => saveDeadline(due, event.target.value)} />{!time && <small>Defaults to 23:59</small>}</label>}
      <label>Notes<textarea rows={3} value={notes} onChange={(event) => setNotes(event.target.value)}
        onBlur={() => { if (notes !== task.notes) save({ field: 'notes', value: notes }); }} /></label>
      {failure && <p className="task-hint" role="alert">{failure}</p>}
      {history.length > 0 && <details className="task-history"><summary>Schedule history</summary><ul>
        {history.map((entry, index) => <li key={`${entry.changedAt}-${index}`}>{entry.previousDate} → {entry.nextDate ?? 'No Date'}{entry.automatic && ' · Rescheduled'}</li>)}
      </ul></details>}
    </div>
  </Modal>;
}
