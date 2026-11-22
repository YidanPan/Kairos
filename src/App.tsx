import { useCallback, useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { DataSettings } from './DataSettings';
import { getStorageInfo, integrationStatus, isDesktop } from './desktop';
import type { Integration } from './desktop';
import { SystemSettings } from './SystemSettings';
import { TaskWorkspace } from './TaskWorkspace';
import { connectPanel, defaultAppearance, resizeHandles, setBackdrop, startPanelGesture } from './panel';
import type { Appearance, Panel, Theme, Transparency } from './panel';

type StorageState = { status: 'loading' | 'ready' } | { status: 'error'; message: string };

const dateFormat = new Intl.DateTimeFormat(undefined, {
  weekday: 'long', month: 'long', day: 'numeric',
});

export function App() {
  const [storage, setStorage] = useState<StorageState>({ status: 'loading' });
  const [attempt, setAttempt] = useState(0);
  const [today, setToday] = useState(() => dateFormat.format(new Date()));
  const [panel, setPanel] = useState<Panel>();
  const [appearance, setAppearance] = useState(defaultAppearance);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [integration, setIntegration] = useState<Integration | null>();
  const [windowError, setWindowError] = useState('');
  const [backdrop, setBackdropActive] = useState(false);
  const [systemDark, setSystemDark] = useState(() => matchMedia('(prefers-color-scheme: dark)').matches);
  const chrome = useRef<HTMLElement>(null);
  const taskContent = useRef<HTMLDivElement>(null);
  const settingsContent = useRef<HTMLDivElement>(null);
  const title = useRef<HTMLHeadingElement>(null);
  const showSettings = useCallback(() => setSettingsOpen(true), []);
  const showTasks = useCallback(() => {
    if (!document.querySelector('.settings-body dialog[open]')) setSettingsOpen(false);
  }, []);
  const panelWindow = isDesktop ? getCurrentWindow() : null;
  const dark = appearance.theme === 'dark' || (appearance.theme === 'system' && systemDark);

  function reportWindowError(error: unknown) {
    setWindowError(error instanceof Error ? error.message : String(error));
  }

  function updateIntegration(update: Partial<Integration>) {
    setIntegration((current) => current ? { ...current, ...update } : current);
  }

  useEffect(() => {
    const media = matchMedia('(prefers-color-scheme: dark)');
    const update = ({ matches }: MediaQueryListEvent) => setSystemDark(matches);
    media.addEventListener('change', update);
    return () => media.removeEventListener('change', update);
  }, []);

  useEffect(() => {
    if (!isDesktop) return;
    const lifetime = new AbortController();
    const releaseTray = listen('kairos-open-settings', showSettings);
    void releaseTray.catch(reportWindowError);
    const releaseInteraction = listen<boolean>('kairos-click-through', (event) => updateIntegration({ clickThrough: event.payload }));
    void releaseInteraction.catch(reportWindowError);
    void connectPanel(lifetime.signal, reportWindowError).then((controller) => {
      if (controller && !lifetime.signal.aborted) {
        setAppearance(controller.appearance);
        setPanel(controller);
      }
    }).catch(reportWindowError);
    return () => {
      lifetime.abort();
      void releaseTray.then((unlisten) => unlisten(), () => undefined);
      void releaseInteraction.then((unlisten) => unlisten(), () => undefined);
    };
  }, [showSettings]);

  useEffect(() => {
    if (!isDesktop) return;
    let active = true;
    void integrationStatus().then(
      (status) => { if (active) setIntegration(status); },
      (error: unknown) => { if (active) { setIntegration(null); reportWindowError(error); } },
    );
    const keyboard = (event: KeyboardEvent) => {
      if (event.ctrlKey && !event.altKey && !event.shiftKey && event.key === ',' && !document.querySelector('dialog[open]')) {
        event.preventDefault();
        showSettings();
      }
    };
    window.addEventListener('keydown', keyboard);
    return () => { active = false; window.removeEventListener('keydown', keyboard); };
  }, [showSettings]);

  useEffect(() => {
    if (!panel) return;
    let active = true;
    void setBackdrop(appearance, dark).then(
      (supported) => { if (active) setBackdropActive(supported); },
      () => { if (active) setBackdropActive(false); },
    );
    return () => { active = false; };
  }, [appearance, dark, panel]);

  useEffect(() => {
    const body = settingsOpen ? settingsContent.current : taskContent.current;
    if (!panel || !chrome.current || !body) return;
    const bar = chrome.current;
    const fit = () => panel.fit(Math.ceil(bar.offsetHeight + body.offsetHeight + 2));
    const observer = new ResizeObserver(fit);
    observer.observe(bar);
    observer.observe(body);
    return () => observer.disconnect();
  }, [panel, settingsOpen]);

  useEffect(() => {
    const refreshDate = () => setToday(dateFormat.format(new Date()));
    const timer = window.setInterval(refreshDate, 30_000);
    window.addEventListener('focus', refreshDate);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener('focus', refreshDate);
    };
  }, []);

  useEffect(() => {
    if (settingsOpen) title.current?.focus();
  }, [settingsOpen]);

  useEffect(() => {
    if (!isDesktop) return;

    let active = true;
    getStorageInfo().then(
      () => { if (active) setStorage({ status: 'ready' }); },
      (error: unknown) => {
        if (active) setStorage({
          status: 'error',
          message: typeof error === 'string' ? error : 'Kairos couldn’t open local storage.',
        });
      },
    );
    return () => { active = false; };
  }, [attempt]);

  function retryStorage() {
    setStorage({ status: 'loading' });
    setAttempt((previous) => previous + 1);
  }

  function updateAppearance(next: Appearance) {
    setAppearance(next);
    void panel?.updateAppearance(next);
  }

  const storageError = storage.status === 'error' && <div className="storage-error">
    <p className="status" role="alert">{storage.message}</p>
    <button type="button" onClick={retryStorage}>Try again</button>
  </div>;
  const windowStatus = windowError && <p className="status" role="alert">{windowError}</p>;

  return (
    <main className="panel" data-theme={dark ? 'dark' : 'light'} data-transparency={appearance.transparency} data-backdrop={backdrop}>
      <header className="panel-header" ref={chrome}>
        {panelWindow && (
          <div className="window-controls" aria-label="Window controls">
            <button className="window-control close" type="button" aria-label="Close to tray" title="Close to tray" onClick={() => void panelWindow.hide().catch(reportWindowError)}><span aria-hidden="true">×</span></button>
            <button className="window-control minimize" type="button" aria-label="Minimize" title="Minimize" onClick={() => void panelWindow.minimize().catch(reportWindowError)}><span aria-hidden="true">−</span></button>
            <button className="window-control expand" type="button" aria-label="Expand or restore" title="Expand or restore" onClick={() => void panel?.toggleExpanded()} disabled={!panel}><span aria-hidden="true">+</span></button>
          </div>
        )}
        <div className="title-region" onPointerDown={(event) => {
          if (panelWindow) startPanelGesture(event, 'Move', reportWindowError);
        }}>
          <h1 ref={title} tabIndex={settingsOpen ? -1 : undefined}>{settingsOpen ? 'Settings' : 'Kairos'}</h1>
        </div>
        <button className="settings-button" type="button" onClick={settingsOpen ? showTasks : showSettings}>{settingsOpen ? 'Back to tasks' : 'Settings'}</button>
      </header>
      <div className="panel-scroll" hidden={settingsOpen}>
        <div className={storage.status === 'ready' ? 'panel-body task-board-body' : 'panel-body'} ref={taskContent}>
          {storage.status !== 'ready' && <><time>{today}</time><p className="tagline">At your own pace.</p></>}
          {!isDesktop && <p className="status">Open the desktop app to access local storage.</p>}
          {isDesktop && storage.status === 'loading' && <p className="status" role="status">Opening local storage…</p>}
          {storageError}
          {windowStatus}
          {isDesktop && storage.status === 'ready' && integration !== undefined && <TaskWorkspace active={!settingsOpen}
            startupPage={integration?.preferences.startupPage} onOpenSettings={showSettings} onShowTasks={showTasks} />}
        </div>
      </div>
      {settingsOpen && <div className="panel-scroll" role="region" aria-label="Settings">
        <div className="panel-body settings-body" ref={settingsContent}>
          {windowStatus}
          <section id="appearance-settings" className="appearance-settings" aria-label="Appearance">
            <h2>Appearance</h2>
            <label>
              Theme
              <select value={appearance.theme} onChange={(event) => updateAppearance({ ...appearance, theme: event.target.value as Theme })}>
                <option value="system">Follow System</option>
                <option value="light">Light</option>
                <option value="dark">Dark</option>
              </select>
            </label>
            <label>
              Transparency
              <select value={appearance.transparency} onChange={(event) => updateAppearance({ ...appearance, transparency: event.target.value as Transparency })}>
                <option value="solid">Solid</option>
                <option value="light">Light</option>
                <option value="medium">Medium</option>
                <option value="transparent">Transparent</option>
              </select>
            </label>
          </section>
          {integration && <SystemSettings system={integration} onUpdate={updateIntegration} />}
          {isDesktop && storage.status === 'ready' && <DataSettings />}
          {storageError}
        </div>
      </div>}
      {panelWindow && resizeHandles.map((direction) => (
        <div key={direction} className="resize-handle" data-direction={direction} aria-hidden="true" onPointerDown={(event) => {
          startPanelGesture(event, direction, reportWindowError);
        }} />
      ))}
    </main>
  );
}
