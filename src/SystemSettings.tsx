import { useState } from 'react';
import { isMac, saveBehavior, setClickThrough, setLaunchAtLogin } from './desktop';
import type { Behavior, Integration, StartupPage, WindowMode } from './desktop';

const shortcuts = [
  ['Show or hide', 'T'], ['New task', 'N'], ['Search', 'F'], ['Settings', 'S'], ['Restore mouse interaction', 'R'],
];

export function SystemSettings({ system, onUpdate }: {
  system: Integration;
  onUpdate: (update: Partial<Integration>) => void;
}) {
  const [whitelist, setWhitelist] = useState(system.preferences.whitelist.join('\n'));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const preferences = system.preferences;

  async function update(action: () => Promise<Partial<Integration>>) {
    setBusy(true);
    try {
      onUpdate(await action());
      setError('');
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  }

  function save(change: Partial<Behavior>) {
    void update(async () => {
      const next = await saveBehavior({ ...preferences, ...change });
      if (change.whitelist) setWhitelist(next.whitelist.join('\n'));
      return { preferences: next };
    });
  }

  return <section className="appearance-settings system-settings" aria-label="Behavior">
    <h2>Behavior</h2>
    <fieldset disabled={busy}>
      <label>Launch at login<input type="checkbox" checked={system.startup ?? false} disabled={system.startup === null}
        onChange={(event) => { const checked = event.target.checked; void update(async () => ({ startup: await setLaunchAtLogin(checked) })); }} /></label>
      <label>Startup page<select value={preferences.startupPage} onChange={(event) => save({ startupPage: event.target.value as StartupPage })}>
        <option value="tasks">Tasks</option><option value="completed">Completed</option><option value="statistics">Statistics</option>
      </select></label>
      <label>Window mode<select value={preferences.windowMode} onChange={(event) => save({ windowMode: event.target.value as WindowMode })}>
        <option value="smart">Smart</option><option value="desktop">{isMac ? 'Normal' : 'Desktop'}</option><option value="alwaysOnTop">Always on Top</option>
      </select></label>
      {preferences.windowMode === 'smart' && <form className="task-form smart-preferences" onSubmit={(event) => {
        event.preventDefault();
        save({ whitelist: whitelist.split('\n') });
      }}>
        <label>Smart mode whitelist<textarea rows={2} value={whitelist} placeholder={isMac ? 'Safari\ncom.apple.TextEdit' : 'example.exe'}
          onChange={(event) => setWhitelist(event.target.value)} /></label>
        <p className="task-hint">{isMac ? 'One app name or bundle ID per line.' : 'One .exe filename per line.'} Leave empty to allow all apps.</p>
        <button type="submit" disabled={whitelist === preferences.whitelist.join('\n')}>Save whitelist</button>
      </form>}
      <label>Click-through<input type="checkbox" checked={system.clickThrough}
        onChange={(event) => { const checked = event.target.checked; void update(async () => ({ clickThrough: await setClickThrough(checked) })); }} /></label>
      <p className="task-hint">Choose Restore Mouse Interaction from the {isMac ? 'Kairos menu bar icon' : 'tray'} to turn click-through off.</p>
    </fieldset>
    <details className="integration-shortcuts">
      <summary>Global shortcuts</summary>
      <dl>{shortcuts.map(([label, key]) => <div key={label}><dt>{label}</dt>
        <dd>{system.unavailableShortcuts.includes(label) ? 'Unavailable' : <kbd>{isMac ? '⌘⌥⇧' : 'Ctrl+Alt+Shift+'}{key}</kbd>}</dd>
      </div>)}</dl>
    </details>
    {system.unavailableShortcuts.length > 0 && <p className="task-hint" role="status">Some shortcuts are unavailable. Use the {isMac ? 'menu bar' : 'tray'} or task panel.</p>}
    {error && <p className="status" role="alert">{error}</p>}
  </section>;
}
