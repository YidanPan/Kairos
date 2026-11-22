import { useCallback, useEffect, useRef, useState } from 'react';
import type { FormEvent } from 'react';
import { listen } from '@tauri-apps/api/event';
import { taskBoard } from './desktop';
import { CategoryReminders, ReminderHub } from './ReminderControls';
import { CategoryOptions, TaskDetails } from './TaskDetails';
import { Modal } from './TaskModal';
import { TaskHistory } from './TaskHistory';
import type { HistoryKind } from './TaskHistory';
import { DailyProgress, TaskStatistics } from './TaskStatistics';
import { activeTasks, emptyFilters, filterTasks, groupTasks, isOverdue, localDay, moveTask, priorities, sections, taskSection, taskStatus } from './taskModel';
import type { Board, Category, Priority, Section, SortMode, Subtask, Task, TaskAction } from './taskModel';

const dateFormat = new Intl.DateTimeFormat(undefined, { weekday: 'long', month: 'long', day: 'numeric' });
const newCategory = (): Category => ({ id: crypto.randomUUID(), name: '', color: '#888888' });

function readableDate(value: string | null) {
  return value ? new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric' }).format(new Date(`${value}T12:00:00`)) : '';
}

function Categories({ categories, run, onClose }: { categories: Category[]; run: (action: TaskAction) => Promise<boolean>; onClose: () => void }) {
  const [draft, setDraft] = useState(newCategory);
  const [removing, setRemoving] = useState(false);
  const [replacement, setReplacement] = useState('');
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);
  const existing = categories.some((category) => category.id === draft.id);

  function select(category: Category) {
    setDraft(category);
    setRemoving(false);
    setReplacement('');
    setFailed(false);
  }

  async function submit(action: TaskAction) {
    setBusy(true);
    const success = await run(action);
    setBusy(false);
    setFailed(!success);
    if (success) select(newCategory());
  }

  return <Modal title="Categories" onClose={onClose}>
    <div className="category-choices" aria-label="Existing categories">
      {categories.map((category) => <button key={category.id} type="button" aria-pressed={draft.id === category.id} onClick={() => select(category)}>
        <span className="category-dot" style={{ backgroundColor: category.color }} />{category.name}
      </button>)}
      {existing && <button type="button" onClick={() => select(newCategory())}>New category</button>}
    </div>
    {removing ? <div className="task-form">
      <label>Move tasks to<select value={replacement} onChange={(event) => setReplacement(event.target.value)}>
        <CategoryOptions categories={categories.filter((category) => category.id !== draft.id)} empty="None" />
      </select></label>
      <div className="task-actions"><button type="button" onClick={() => setRemoving(false)}>Cancel</button>
        <button type="button" className="task-danger" disabled={busy} onClick={() => void submit({ kind: 'deleteCategory', id: draft.id, replacement: replacement || null })}>Remove category</button></div>
    </div> : <form className="task-form" onSubmit={(event) => { event.preventDefault(); void submit({ kind: 'saveCategory', ...draft }); }}>
      <div className="category-edit">
        <label>Category name<input value={draft.name} onChange={(event) => setDraft({ ...draft, name: event.target.value })} required /></label>
        <label>Color<input type="color" value={draft.color} onChange={(event) => setDraft({ ...draft, color: event.target.value })} /></label>
      </div>
      <div className="task-actions">
        {existing && <button type="button" onClick={() => setRemoving(true)}>Remove…</button>}
        <button type="submit" className="task-primary" disabled={busy || !draft.name.trim()}>{existing ? 'Save category' : 'Add category'}</button>
      </div>
    </form>}
    {existing && !removing && <CategoryReminders key={draft.id} id={draft.id} />}
    {failed && <p className="task-hint" role="alert">The category change could not be saved. Check its name and try again.</p>}
  </Modal>;
}

function TaskRow({ task, category, subtasks, now, manual, onOpen, onToggle, onDrop, onMove }: {
  task: Task; category?: Category; subtasks: Subtask[]; now: Date; manual: boolean; onOpen: () => void; onToggle: () => void;
  onDrop: (id: string) => void; onMove: (offset: number) => void;
}) {
  const status = taskStatus(task, now);
  return <li className="task-row" draggable={manual} onDragStart={(event) => {
    event.dataTransfer.setData('text/plain', task.id);
    event.dataTransfer.effectAllowed = 'move';
  }} onDragOver={(event) => { if (manual) event.preventDefault(); }} onDrop={(event) => {
    if (!manual) return;
    event.preventDefault();
    onDrop(event.dataTransfer.getData('text/plain'));
  }}>
    <input className="task-check" type="checkbox" checked={Boolean(task.completedAt)} aria-label={`${task.completedAt ? 'Restore' : 'Complete'} ${task.title}`} onChange={onToggle} />
    <button type="button" className="task-title" onClick={onOpen}>{task.title}</button>
    <div className="task-indicators">
      {status && <span className="task-state" data-overdue={isOverdue(task, now)}>{status}</span>}
      {subtasks.length > 0 && <span className="task-state" aria-label="Subtask progress">{subtasks.filter((item) => item.completedAt).length}/{subtasks.length}</span>}
    </div>
    {manual && <div className="task-reorder"><button type="button" aria-label={`Move ${task.title} up`} onClick={() => onMove(-1)}>↑</button>
      <button type="button" aria-label={`Move ${task.title} down`} onClick={() => onMove(1)}>↓</button></div>}
    <div className="task-metadata">
      <span data-priority={task.priority}>{task.priority}</span>
      {category && <span><i className="category-dot" style={{ backgroundColor: category.color }} />{category.name}</span>}
      {task.scheduledDate && <span>Scheduled {readableDate(task.scheduledDate)}</span>}
      {task.dueDate && <span>Due {readableDate(task.dueDate)} · {task.dueTime ?? '23:59'}</span>}
    </div>
  </li>;
}

export function TaskWorkspace({ onOpenSettings }: { onOpenSettings: () => void }) {
  const [board, setBoard] = useState<Board>();
  const [now, setNow] = useState(() => new Date());
  const [error, setError] = useState('');
  const [quickAdd, setQuickAdd] = useState(false);
  const [newTitle, setNewTitle] = useState('');
  const [saving, setSaving] = useState(false);
  const [categoriesOpen, setCategoriesOpen] = useState(false);
  const [statisticsOpen, setStatisticsOpen] = useState(false);
  const [detailId, setDetailId] = useState<string>();
  const [view, setView] = useState<'tasks' | HistoryKind>('tasks');
  const [filters, setFilters] = useState(emptyFilters);
  const [filterOpen, setFilterOpen] = useState(false);
  const [searchOpen, setSearchOpen] = useState(false);
  const [query, setQuery] = useState('');
  const queue = useRef(Promise.resolve());
  const addInput = useRef<HTMLInputElement>(null);
  const searchInput = useRef<HTMLInputElement>(null);
  const moreMenu = useRef<HTMLDetailsElement>(null);

  const run = useCallback((action?: TaskAction) => {
    const result = queue.current.then(async () => {
      const next = await taskBoard(action);
      setBoard(next);
      setNow(new Date());
      if (action) setError('');
      return true;
    }).catch((reason: unknown) => {
      setError(typeof reason === 'string' ? reason : 'Kairos could not save or load tasks.');
      return false;
    });
    queue.current = result.then(() => undefined);
    return result;
  }, []);

  useEffect(() => {
    const refresh = () => { void run(); };
    refresh();
    const updates = listen('kairos-reminders', refresh);
    const opens = listen<string>('kairos-open-task', (event) => { refresh(); setDetailId(event.payload); });
    const timer = window.setInterval(refresh, 30_000);
    window.addEventListener('focus', refresh);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener('focus', refresh);
      for (const registration of [updates, opens]) void registration.then((unlisten) => unlisten(), () => undefined);
    };
  }, [run]);

  useEffect(() => {
    if (quickAdd) addInput.current?.focus();
  }, [quickAdd]);

  useEffect(() => {
    if (searchOpen && view === 'tasks') searchInput.current?.focus();
  }, [searchOpen, view]);

  useEffect(() => {
    const open = (event: KeyboardEvent) => {
      if (!event.ctrlKey || document.querySelector('dialog[open]')) return;
      const key = event.key.toLowerCase();
      if (key === 'n') {
        event.preventDefault();
        setQuickAdd(true);
        addInput.current?.focus();
      }
      if (key === 'f') {
        event.preventDefault();
        setView('tasks');
        setSearchOpen(true);
        searchInput.current?.focus();
      }
    };
    window.addEventListener('keydown', open);
    return () => window.removeEventListener('keydown', open);
  }, []);

  async function add(event: FormEvent) {
    event.preventDefault();
    const title = newTitle.trim();
    if (!title || saving) return;
    setSaving(true);
    if (await run({ kind: 'create', id: crypto.randomUUID(), title })) {
      setNewTitle('');
      setQuickAdd(false);
      setView('tasks');
    }
    setSaving(false);
  }

  if (!board) return <div className="workspace-message" role={error ? 'alert' : 'status'}>
    {error || 'Loading tasks…'}{error && <button type="button" onClick={() => { setError(''); void run(); }}>Try again</button>}
  </div>;

  const detail = board.tasks.find((task) => task.id === detailId);
  const categoriesById = new Map(board.categories.map((category) => [category.id, category]));
  const filterCategory = categoriesById.has(filters.categoryId) ? filters.categoryId : '';
  const currentFilters = { ...filters, categoryId: filterCategory };
  const filtering = Object.values(currentFilters).some(Boolean);
  const visible = filterTasks(board.tasks, query, currentFilters, now);
  const grouped = groupTasks(visible, board.sortMode, now);
  const totalToday = board.statistics.completedToday + activeTasks(board.tasks).filter((task) => taskSection(task, now) === 'Today').length;
  const archives = {
    completed: board.tasks.filter((task) => task.completedAt && !task.trashedAt),
    trash: board.tasks.filter((task) => task.trashedAt),
    skipped: board.tasks.filter((task) => task.skippedAt && !task.trashedAt),
  };
  const subtasksByTask = new Map<string, Subtask[]>();
  for (const item of board.subtasks) {
    const items = subtasksByTask.get(item.taskId);
    if (items) items.push(item);
    else subtasksByTask.set(item.taskId, [item]);
  }
  const hour = now.getHours();
  const greeting = hour < 12 ? 'Good morning.' : hour < 18 ? 'Good afternoon.' : 'Good evening.';
  const today = localDay(now);

  function chooseMore(action: () => void) {
    if (moreMenu.current) moreMenu.current.open = false;
    action();
  }

  function reorder(section: Section, source: string, target: string) {
    const ids = moveTask(grouped[section], source, target, now);
    if (ids) void run({ kind: 'reorder', ids });
  }

  function row(task: Task, section?: Section, index = 0) {
    return <TaskRow key={task.id} task={task} now={now} category={categoriesById.get(task.categoryId ?? '')} subtasks={subtasksByTask.get(task.id) ?? []}
      manual={board?.sortMode === 'manual' && Boolean(section)} onOpen={() => setDetailId(task.id)}
      onToggle={() => void run({ kind: 'setCompleted', id: task.id, completed: !task.completedAt })}
      onDrop={(source) => { if (section) reorder(section, source, task.id); }}
      onMove={(offset) => {
        if (section && grouped[section][index + offset]) reorder(section, task.id, grouped[section][index + offset].id);
      }} />;
  }

  return <div className="task-workspace">
    <div className="task-toolbar">
      <div className="workspace-summary"><div><time dateTime={today}>{dateFormat.format(now)}</time><h2>{greeting}</h2></div>
        <button type="button" className="quick-add-button" aria-label="New task" aria-expanded={quickAdd} onClick={() => setQuickAdd((open) => !open)}>+</button>
      </div>
      {quickAdd && <form className="quick-add-form" onSubmit={(event) => void add(event)}>
        <input ref={addInput} aria-label="New task title" placeholder="New task…" value={newTitle} onChange={(event) => setNewTitle(event.target.value)}
          onKeyDown={(event) => { if (event.key === 'Escape') { setQuickAdd(false); setNewTitle(''); } }} />
        <button type="submit" disabled={saving || !newTitle.trim()}>Add</button>
      </form>}
      <DailyProgress completed={board.statistics.completedToday} total={totalToday} />
      <div className="task-controls">
        {view === 'tasks' ? <>
          <button type="button" aria-expanded={searchOpen} onClick={() => {
            if (searchOpen) setQuery('');
            setSearchOpen(!searchOpen);
          }}>Search</button>
          <div className="task-filter-anchor">
            <button type="button" aria-expanded={filterOpen} aria-controls="task-filters" onClick={() => setFilterOpen(!filterOpen)}>Filter{filtering && ' ·'}</button>
            <div id="task-filters" className="task-control-filters" role="group" aria-label="Task filters" hidden={!filterOpen}
              onKeyDown={(event) => { if (event.key === 'Escape') setFilterOpen(false); }}>
              <label>Category<select aria-label="Filter by category" value={filterCategory} onChange={(event) => setFilters({ ...filters, categoryId: event.target.value })}><CategoryOptions categories={board.categories} empty="All categories" /></select></label>
              <label>Priority<select aria-label="Filter by priority" value={filters.priority} onChange={(event) => setFilters({ ...filters, priority: event.target.value as Priority | '' })}>
                <option value="">All priorities</option>{priorities.map((priority) => <option key={priority} value={priority}>{priority[0].toUpperCase() + priority.slice(1)}</option>)}
              </select></label>
              <label>Scheduled date<input type="date" value={filters.scheduledDate} onChange={(event) => setFilters({ ...filters, scheduledDate: event.target.value })} /></label>
              <label>Due date<input type="date" value={filters.dueDate} onChange={(event) => setFilters({ ...filters, dueDate: event.target.value })} /></label>
              <label>Overdue<select value={filters.overdue} onChange={(event) => setFilters({ ...filters, overdue: event.target.value as typeof filters.overdue })}>
                <option value="">Any</option><option value="yes">Overdue</option><option value="no">Not overdue</option>
              </select></label>
              <button type="button" disabled={!filtering} onClick={() => setFilters(emptyFilters)}>Clear all</button>
            </div>
          </div>
        </> : <button type="button" onClick={() => setView('tasks')}>Tasks</button>}
        <details className="workspace-more" ref={moreMenu} onKeyDown={(event) => {
          if (event.key === 'Escape') chooseMore(() => moreMenu.current?.querySelector('summary')?.focus());
        }}>
          <summary>More</summary>
          <div className="workspace-more-actions">
            <button type="button" onClick={() => chooseMore(() => setView('completed'))}>Completed</button>
            <button type="button" onClick={() => chooseMore(() => setStatisticsOpen(true))}>Statistics</button>
            <button type="button" onClick={() => chooseMore(() => setView('trash'))}>Trash</button>
            {archives.skipped.length > 0 && <button type="button" onClick={() => chooseMore(() => setView('skipped'))}>Skipped</button>}
            <button type="button" onClick={() => chooseMore(() => setCategoriesOpen(true))}>Categories</button>
            <button type="button" onClick={() => chooseMore(onOpenSettings)}>Settings</button>
          </div>
        </details>
        <ReminderHub openTask={setDetailId} completeTask={(id) => run({ kind: 'setCompleted', id, completed: true })} />
        <select aria-label="Task order" value={board.sortMode} onChange={(event) => void run({ kind: 'setSort', mode: event.target.value as SortMode })}>
          <option value="automatic">Automatic</option><option value="manual">Manual</option>
        </select>
      </div>
      {searchOpen && view === 'tasks' && <input type="search" ref={searchInput} className="task-search" aria-label="Search task titles and notes" placeholder="Search tasks…"
        value={query} onChange={(event) => setQuery(event.target.value)} onKeyDown={(event) => {
          if (event.key === 'Escape') { setQuery(''); setSearchOpen(false); }
        }} />}
    </div>
    {error && <p className="task-hint task-error" role="alert">{error}<button type="button" aria-label="Dismiss error" onClick={() => setError('')}>×</button></p>}
    {view === 'tasks' ? sections.map((section) => <section className="task-section" key={section}><h3>{section}</h3>
      {grouped[section].length ? <ul>{grouped[section].map((task, index) => row(task, section, index))}</ul> : <p className="task-hint">No tasks</p>}
    </section>) : <TaskHistory key={view} kind={view} items={archives[view]} run={run} openTask={setDetailId} />}
    {detail && <TaskDetails key={detail.id} task={detail} categories={board.categories} history={board.scheduleHistory.filter((entry) => entry.taskId === detail.id)}
      subtasks={subtasksByTask.get(detail.id) ?? []} run={run} onClose={() => setDetailId(undefined)} />}
    {categoriesOpen && <Categories categories={board.categories} run={run} onClose={() => setCategoriesOpen(false)} />}
    {statisticsOpen && <TaskStatistics statistics={board.statistics} totalToday={totalToday} run={run} onClose={() => setStatisticsOpen(false)} />}
  </div>;
}
