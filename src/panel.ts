import { invoke } from '@tauri-apps/api/core';
import { LogicalSize, PhysicalPosition, availableMonitors, currentMonitor, getCurrentWindow, primaryMonitor } from '@tauri-apps/api/window';
import type { Monitor } from '@tauri-apps/api/window';
import type { PointerEvent } from 'react';

export type Theme = 'system' | 'light' | 'dark';
export type Transparency = 'solid' | 'light' | 'medium' | 'transparent';
export interface Appearance { theme: Theme; transparency: Transparency }

interface Layout { left: number; top: number; width: number; height: number }
interface Preferences extends Appearance { display: string | null; layouts: Record<string, Layout> }

export const defaultAppearance: Appearance = { theme: 'system', transparency: 'light' };
const minimumWidth = 320;
const minimumHeight = 250;
const defaultWidth = 440;

export const resizeHandles = ['North', 'South', 'East', 'West', 'NorthEast', 'NorthWest', 'SouthEast', 'SouthWest'] as const;
type Gesture = 'Move' | typeof resizeHandles[number];

function monitorKey(monitor: Monitor) {
  return monitor.name ?? `${monitor.position.x},${monitor.position.y}`;
}

function monitorBounds(monitor: Monitor) {
  const { scaleFactor, workArea } = monitor;
  const width = workArea.size.width / scaleFactor;
  const height = workArea.size.height / scaleFactor;
  return { width, height, maxHeight: Math.floor(height * .7) };
}

function clamp(value: number, minimum: number, maximum: number) {
  return Math.min(Math.max(value, minimum), maximum);
}

export function startPanelGesture(event: PointerEvent<HTMLElement>, direction: Gesture, report: (error: unknown) => void) {
  if (event.button !== 0 || !event.isPrimary) return;
  event.preventDefault();
  const target = event.currentTarget;
  const pointer = event.pointerId;
  const start = { x: event.screenX, y: event.screenY };
  let pending: typeof start | undefined;
  let applying = false;
  const window = getCurrentWindow();
  const origin = Promise.all([window.outerPosition(), window.innerSize(), currentMonitor()]);
  void origin.catch(report);

  function move(event: globalThis.PointerEvent) {
    if (event.pointerId !== pointer) return;
    pending = { x: event.screenX - start.x, y: event.screenY - start.y };
    if (applying) return;
    applying = true;
    void apply().catch(report).finally(() => { applying = false; });
  }

  async function apply() {
    const [position, size, monitor] = await origin;
    if (!monitor) return;
    const { scaleFactor: scale } = monitor;
    const bounds = monitorBounds(monitor);
    while (pending) {
      const delta = pending;
      pending = undefined;
      if (direction === 'Move') {
        await window.setPosition(new PhysicalPosition(position.x + Math.round(delta.x * scale), position.y + Math.round(delta.y * scale)));
        continue;
      }
      const width = size.width / scale;
      const height = size.height / scale;
      const left = direction.includes('West');
      const top = direction.includes('North');
      const horizontal = left ? -delta.x : direction.includes('East') ? delta.x : 0;
      const vertical = top ? -delta.y : direction.includes('South') ? delta.y : 0;
      const nextWidth = clamp(width + horizontal, minimumWidth, bounds.width);
      const nextHeight = clamp(height + vertical, minimumHeight, bounds.maxHeight);
      await window.setSize(new LogicalSize(nextWidth, nextHeight));
      if (left || top) {
        await window.setPosition(new PhysicalPosition(
          position.x + (left ? Math.round((width - nextWidth) * scale) : 0),
          position.y + (top ? Math.round((height - nextHeight) * scale) : 0),
        ));
      }
    }
  }

  target.setPointerCapture(pointer);
  target.addEventListener('pointermove', move);
  target.addEventListener('lostpointercapture', () => target.removeEventListener('pointermove', move), { once: true });
}

export function setBackdrop(appearance: Appearance, dark: boolean) {
  const opacity = { solid: 0, light: 198, medium: 150, transparent: 88 }[appearance.transparency];
  const base = dark ? 23 : 245;
  return invoke<boolean>('set_panel_backdrop', { tint: opacity ? [base, base, base, opacity] : null });
}

export async function connectPanel(signal: AbortSignal, report: (error: unknown) => void) {
  const window = getCurrentWindow();
  let preferences: Preferences = { ...defaultAppearance, display: null, layouts: {} };
  let lastTopology = '';
  let contentHeight = minimumHeight;
  let unexpandedHeight: number | null = null;
  let suppressGeometryUntil = 0;
  let geometryTimer = 0;
  let queue = Promise.resolve();
  let observing = false;

  function schedule(operation: () => Promise<void>) {
    queue = queue.then(async () => {
      if (!signal.aborted) await operation();
    }).catch(report);
    return queue;
  }

  async function persist() {
    await invoke('save_panel_preferences', { preferences });
  }

  async function constrain(monitor: Monitor) {
    const { maxHeight } = monitorBounds(monitor);
    const maxWidth = Math.floor(monitor.workArea.size.width / monitor.scaleFactor);
    await window.setSizeConstraints({
      minWidth: Math.min(minimumWidth, maxWidth),
      minHeight: Math.min(minimumHeight, maxHeight),
      maxHeight,
      maxWidth,
    });
  }

  async function restore(monitor: Monitor) {
    suppressGeometryUntil = performance.now() + 550;
    const bounds = monitorBounds(monitor);
    const saved = preferences.layouts[monitorKey(monitor)];
    const width = clamp(saved?.width ?? defaultWidth, minimumWidth, bounds.width);
    const height = clamp(Math.max(saved?.height ?? minimumHeight, contentHeight), minimumHeight, bounds.maxHeight);
    const left = clamp(saved?.left ?? bounds.width - width - 22, 0, bounds.width - width);
    const top = clamp(saved?.top ?? 22, 0, bounds.height - height);
    const { scaleFactor: scale, workArea } = monitor;
    await window.setPosition(new PhysicalPosition(
      workArea.position.x + Math.round(left * scale),
      workArea.position.y + Math.round(top * scale),
    ));
    await constrain(monitor);
    await window.setSize(new LogicalSize(width, height));
    unexpandedHeight = null;
  }

  async function checkDisplays() {
    const displays = await availableMonitors();
    if (!displays.length) {
      lastTopology = '';
      return false;
    }
    if (await window.isMinimized()) return false;
    const topology = JSON.stringify([...displays]
      .sort((left, right) => monitorKey(left).localeCompare(monitorKey(right)))
      .map((display) => [monitorKey(display), display.workArea, display.scaleFactor]));
    if (topology === lastTopology) return true;
    lastTopology = '';
    const preferred = displays.find((display) => monitorKey(display) === preferences.display);
    const screen = preferred ?? await primaryMonitor() ?? displays[0];
    await restore(screen);
    lastTopology = topology;
    return false;
  }

  async function saveGeometry() {
    const monitor = await currentMonitor();
    if (!monitor || await window.isMinimized()) return;
    const display = monitorKey(monitor);
    if (preferences.display !== display) {
      await constrain(monitor);
      if (preferences.layouts[display]) {
        preferences = { ...preferences, display };
        await restore(monitor);
        await persist();
        return;
      }
    }
    const [position, size] = await Promise.all([window.outerPosition(), window.innerSize()]);
    const scale = monitor.scaleFactor;
    const layout = {
      left: (position.x - monitor.workArea.position.x) / scale,
      top: (position.y - monitor.workArea.position.y) / scale,
      width: size.width / scale,
      height: unexpandedHeight ?? size.height / scale,
    };
    preferences = { ...preferences, display, layouts: { ...preferences.layouts, [display]: layout } };
    await persist();
  }

  function geometryChanged() {
    if (!observing || performance.now() < suppressGeometryUntil) return;
    globalThis.clearTimeout(geometryTimer);
    geometryTimer = globalThis.setTimeout(() => {
      void schedule(async () => {
        if (await checkDisplays()) await saveGeometry();
      });
    }, 180);
  }

  try {
    const saved = await invoke<Preferences | null>('load_panel_preferences');
    if (saved) preferences = { ...preferences, ...saved };
  } catch (error) {
    report(error);
  }
  if (signal.aborted) return;

  const cleanup: Array<() => void> = [];
  signal.addEventListener('abort', () => {
    globalThis.clearTimeout(geometryTimer);
    for (const release of cleanup) release();
  }, { once: true });

  try {
    for (const register of [window.onMoved.bind(window), window.onResized.bind(window), window.onScaleChanged.bind(window)]) {
      const release = await register(geometryChanged);
      if (signal.aborted) { release(); return; }
      cleanup.push(release);
    }
    const interval = globalThis.setInterval(() => { void schedule(async () => { await checkDisplays(); }); }, 2500);
    cleanup.push(() => globalThis.clearInterval(interval));
    observing = true;
    await checkDisplays();
  } catch (error) {
    report(error);
  } finally {
    if (!signal.aborted) await window.show();
  }

  return {
    appearance: { theme: preferences.theme, transparency: preferences.transparency },
    updateAppearance(appearance: Appearance) {
      return schedule(async () => {
        preferences = { ...preferences, ...appearance };
        await persist();
      });
    },
    fit(height: number) {
      if (contentHeight === height) return;
      contentHeight = height;
      void schedule(async () => {
        const monitor = await currentMonitor();
        if (!monitor || unexpandedHeight !== null || await window.isMinimized()) return;
        const size = (await window.innerSize()).toLogical(monitor.scaleFactor);
        const manual = preferences.layouts[monitorKey(monitor)]?.height ?? minimumHeight;
        const desired = clamp(Math.max(manual, contentHeight), minimumHeight, monitorBounds(monitor).maxHeight);
        if (Math.abs(size.height - desired) < 1) return;
        suppressGeometryUntil = performance.now() + 350;
        await window.setSize(new LogicalSize(size.width, desired));
      });
    },
    toggleExpanded() {
      return schedule(async () => {
        const monitor = await currentMonitor();
        if (!monitor) return;
        const size = (await window.innerSize()).toLogical(monitor.scaleFactor);
        const maxHeight = monitorBounds(monitor).maxHeight;
        const target = unexpandedHeight === null ? maxHeight : Math.min(maxHeight, Math.max(contentHeight, unexpandedHeight));
        const top = (await window.outerPosition()).y;
        const bottom = monitor.workArea.position.y + monitor.workArea.size.height;
        const overflow = Math.max(0, Math.round(top + target * monitor.scaleFactor - bottom));
        suppressGeometryUntil = performance.now() + 350;
        if (overflow) {
          const { x } = await window.outerPosition();
          await window.setPosition(new PhysicalPosition(x, top - overflow));
        }
        await window.setSize(new LogicalSize(size.width, target));
        unexpandedHeight = unexpandedHeight === null ? size.height : null;
      });
    },
  };
}

export type Panel = NonNullable<Awaited<ReturnType<typeof connectPanel>>>;
