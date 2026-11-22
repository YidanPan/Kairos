import { useId, useState } from 'react';
import { Modal } from './TaskModal';
import type { Statistics, TaskAction } from './taskModel';

const shortDay = new Intl.DateTimeFormat(undefined, { weekday: 'short' });
const fullDay = new Intl.DateTimeFormat(undefined, { weekday: 'long' });

export function DailyProgress({ completed, total }: { completed: number; total: number }) {
  const id = useId();
  const percent = total ? Math.round(100 * completed / total) : 0;
  return <div className="daily-progress">
    <label htmlFor={id}>Today <strong>{completed}/{total} complete</strong> <span>{percent}%</span></label>
    <progress id={id} value={completed} max={total || 1} />
  </div>;
}

export function TaskStatistics({ statistics, totalToday, run, onClose }: {
  statistics: Statistics; totalToday: number; run: (action: TaskAction) => Promise<boolean>; onClose: () => void;
}) {
  const [restDays, setRestDays] = useState(statistics.restDays);
  const [saving, setSaving] = useState(false);
  const [failed, setFailed] = useState(false);
  const peak = Math.max(1, ...statistics.week.map((day) => day.completed));
  const dirty = restDays.length !== statistics.restDays.length || restDays.some((day) => !statistics.restDays.includes(day));

  async function save() {
    setSaving(true);
    setFailed(!(await run({ kind: 'setRestDays', days: restDays })));
    setSaving(false);
  }

  return <Modal title="Statistics" onClose={onClose}>
    <div className="task-statistics">
      <DailyProgress completed={statistics.completedToday} total={totalToday} />
      <section aria-labelledby="weekly-statistics-heading">
        <h3 id="weekly-statistics-heading">This week <span>{statistics.weeklyTotal} completed</span></h3>
        <ul className="statistics-week">
          {statistics.week.map((day) => {
            const timestamp = new Date(`${day.date}T12:00:00`);
            return <li key={day.date} aria-label={`${fullDay.format(timestamp)}, ${day.completed} completed`}>
              <span>{day.completed}</span>
              <div className="statistics-bar-track" aria-hidden="true"><i style={{ height: `${day.completed / peak * 100}%` }} /></div>
              <time dateTime={day.date}>{shortDay.format(timestamp)}</time>
            </li>;
          })}
        </ul>
      </section>
      <dl className="statistics-streak">
        <div><dt>Current streak</dt><dd>{statistics.currentStreak} days</dd></div>
        <div><dt>Best streak</dt><dd>{statistics.bestStreak} days</dd></div>
      </dl>
      <form className="task-form" onSubmit={(event) => { event.preventDefault(); void save(); }}>
        <fieldset disabled={saving} className="statistics-rest-days">
          <legend>Weekly rest days</legend>
          {statistics.week.map((day, index) => {
            const timestamp = new Date(`${day.date}T12:00:00`);
            return <label key={index}>
              <input type="checkbox" aria-label={fullDay.format(timestamp)} checked={restDays.includes(index)}
                onChange={(event) => setRestDays(event.target.checked ? [...restDays, index] : restDays.filter((value) => value !== index))} />
              {shortDay.format(timestamp)}
            </label>;
          })}
        </fieldset>
        <div className="task-actions"><button className="task-primary" type="submit" disabled={saving || !dirty}>Save rest days</button></div>
        {failed && <p className="task-hint" role="alert">Rest days could not be saved.</p>}
      </form>
    </div>
  </Modal>;
}
