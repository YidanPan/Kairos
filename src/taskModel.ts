export type Priority = 'high' | 'medium' | 'low';
export type SortMode = 'automatic' | 'manual';
export type Section = 'Today' | 'Upcoming' | 'No Date';
export const sections: readonly Section[] = ['Today', 'Upcoming', 'No Date'];
export const priorities: readonly Priority[] = ['high', 'medium', 'low'];
export type EditScope = 'this' | 'future' | 'entire';
export type Frequency = 'days' | 'weeks' | 'months';
export interface RecurrenceRule {
  frequency: Frequency;
  interval: number;
  weekdays: number[];
  monthDay: number | null;
  endDate: string | null;
}
export interface Subtask { id: string; taskId: string; title: string; completedAt: string | null }

export interface Task {
  id: string;
  title: string;
  categoryId: string | null;
  priority: Priority;
  scheduledDate: string | null;
  dueDate: string | null;
  dueTime: string | null;
  notes: string;
  createdAt: string;
  completedAt: string | null;
  trashedAt: string | null;
  rescheduledOn: string | null;
  manualOrder: number;
  seriesId: string | null;
  originSeriesId: string | null;
  occurrenceOn: string | null;
  skippedAt: string | null;
  recurrence: RecurrenceRule | null;
}

export interface Category { id: string; name: string; color: string }
export interface ScheduleChange {
  taskId: string;
  previousDate: string;
  nextDate: string | null;
  changedAt: string;
  automatic: boolean;
}
export interface Board {
  tasks: Task[];
  categories: Category[];
  scheduleHistory: ScheduleChange[];
  sortMode: SortMode;
  subtasks: Subtask[];
  statistics: Statistics;
}

export interface Statistics {
  completedToday: number;
  week: { date: string; completed: number }[];
  weeklyTotal: number;
  currentStreak: number;
  bestStreak: number;
  restDays: number[];
}

export interface TaskFilters {
  categoryId: string;
  priority: Priority | '';
  scheduledDate: string;
  dueDate: string;
  overdue: '' | 'yes' | 'no';
}

export const emptyFilters: TaskFilters = { categoryId: '', priority: '', scheduledDate: '', dueDate: '', overdue: '' };

export type Change =
  | { field: 'title' | 'notes'; value: string }
  | { field: 'category' | 'scheduledDate'; value: string | null }
  | { field: 'priority'; value: Priority }
  | { field: 'deadline'; value: { date: string | null; time: string | null } };

export type TaskAction =
  | { kind: 'create'; id: string; title: string }
  | { kind: 'edit'; id: string; change: Change }
  | { kind: 'setCompleted'; id: string; completed: boolean }
  | ({ kind: 'saveCategory' } & Category)
  | { kind: 'deleteCategory'; id: string; replacement: string | null }
  | { kind: 'setSort'; mode: SortMode }
  | { kind: 'setRestDays'; days: number[] }
  | { kind: 'reorder'; ids: string[] }
  | { kind: 'editOccurrence'; id: string; change: Change; scope: EditScope }
  | { kind: 'saveRecurrence'; id: string; rule: RecurrenceRule | null; scope: EditScope }
  | { kind: 'skipOccurrence'; id: string }
  | { kind: 'trashTask'; id: string; scope: EditScope }
  | { kind: 'trashCompleted' | 'purgeTrash'; ids: string[] | null }
  | { kind: 'restoreTrash'; ids: string[] }
  | { kind: 'saveSubtask'; taskId: string; id: string; title: string }
  | { kind: 'setSubtaskCompleted'; taskId: string; id: string; completed: boolean }
  | { kind: 'removeSubtask'; taskId: string; id: string }
  | { kind: 'moveSubtask'; taskId: string; id: string; neighborId: string };

export function localDay(date: Date) {
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${date.getFullYear()}-${month}-${day}`;
}

export function deadline(task: Task) {
  return task.dueDate ? new Date(`${task.dueDate}T${task.dueTime ?? '23:59'}:00`).getTime() : Infinity;
}

export function isOverdue(task: Task, now: Date) {
  return !task.completedAt && deadline(task) <= now.getTime();
}

export function activeTasks(tasks: Task[]) {
  return tasks.filter((task) => !task.completedAt && !task.trashedAt && !task.skippedAt);
}

export function filterTasks(tasks: Task[], query: string, filters: TaskFilters, now: Date) {
  const searchable = (value: string) => value.normalize('NFKC').toLocaleLowerCase();
  const term = searchable(query.trim());
  return activeTasks(tasks).filter((task) => (
    (!term || searchable(task.title).includes(term) || searchable(task.notes).includes(term))
    && (!filters.categoryId || task.categoryId === filters.categoryId)
    && (!filters.priority || task.priority === filters.priority)
    && (!filters.scheduledDate || task.scheduledDate === filters.scheduledDate)
    && (!filters.dueDate || task.dueDate === filters.dueDate)
    && (!filters.overdue || isOverdue(task, now) === (filters.overdue === 'yes'))
  ));
}

export function taskSection(task: Task, now: Date, today = localDay(now)): Section {
  if (isOverdue(task, now) || (task.scheduledDate && task.scheduledDate <= today)) return 'Today';
  return task.scheduledDate ? 'Upcoming' : 'No Date';
}

export function taskStatus(task: Task, now: Date, today = localDay(now)) {
  if (isOverdue(task, now) && task.dueDate) {
    const days = Math.round((Date.parse(today) - Date.parse(task.dueDate)) / 86_400_000);
    return days > 0 ? `Overdue ${days} ${days === 1 ? 'day' : 'days'}` : 'Overdue';
  }
  if (task.rescheduledOn === today || (task.scheduledDate && task.scheduledDate < today)) return 'Rescheduled';
  return '';
}

export function groupTasks(tasks: Task[], mode: SortMode, now: Date) {
  const grouped: Record<Section, Task[]> = { Today: [], Upcoming: [], 'No Date': [] };
  const today = localDay(now);
  for (const task of tasks) grouped[taskSection(task, now, today)].push(task);

  for (const section of sections) grouped[section].sort((left, right) => {
    const overdue = Number(isOverdue(right, now)) - Number(isOverdue(left, now));
    if (overdue) return overdue;
    if (mode === 'manual') return left.manualOrder - right.manualOrder;
    if (section === 'Upcoming' && left.scheduledDate !== right.scheduledDate) {
      return (left.scheduledDate ?? '').localeCompare(right.scheduledDate ?? '');
    }
    const leftDeadline = deadline(left);
    const rightDeadline = deadline(right);
    if (leftDeadline !== rightDeadline) return leftDeadline < rightDeadline ? -1 : 1;
    return priorities.indexOf(left.priority) - priorities.indexOf(right.priority)
      || left.manualOrder - right.manualOrder;
  });
  return grouped;
}

export function moveTask(tasks: Task[], source: string, target: string, now: Date) {
  const from = tasks.findIndex((task) => task.id === source);
  const to = tasks.findIndex((task) => task.id === target);
  if (from < 0 || to < 0 || from === to || isOverdue(tasks[from], now) !== isOverdue(tasks[to], now)) return null;
  const ordered = tasks.map((task) => task.id);
  const [id] = ordered.splice(from, 1);
  ordered.splice(to, 0, id);
  return ordered;
}
