import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Modal } from './TaskModal';

interface BackupFile { name: string; size: number; modifiedAt: number }
interface BackupStatus { database: string; directory: string; customDirectory: boolean; backups: BackupFile[] }
interface RestorePreview { id: string; name: string; currentTasks: number; backupTasks: number; mergeAdditions: number }
type Mode = 'merge' | 'replace';

const timestamp = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'medium' });
const size = new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 });
const sizeLabel = (bytes: number) => bytes >= 1_048_576 ? `${size.format(bytes / 1_048_576)} MB` : `${size.format(bytes / 1024)} KB`;
const taskCount = (count: number) => `${count} ${count === 1 ? 'task' : 'tasks'}`;

export function DataSettings() {
  const [open, setOpen] = useState(false);
  const [status, setStatus] = useState<BackupStatus>();
  const [preview, setPreview] = useState<RestorePreview>();
  const [mode, setMode] = useState<Mode>('merge');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [message, setMessage] = useState('');

  useEffect(() => {
    if (!open) return;
    let active = true;
    void invoke<BackupStatus>('backup_status').then(
      (data) => { if (active) { setStatus(data); setError(''); } },
      (reason: unknown) => { if (active) setError(String(reason)); },
    );
    return () => { active = false; };
  }, [open]);

  async function perform(action: () => Promise<void>) {
    setBusy(true);
    setError('');
    setMessage('');
    try { await action(); }
    catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  }

  function saveDirectory(reset: boolean) {
    void perform(async () => { setStatus(await invoke<BackupStatus>('set_backup_directory', { reset })); });
  }

  function review(name: string | null) {
    void perform(async () => {
      const selected = await invoke<RestorePreview | null>('preview_restore', { name });
      if (selected) { setMode('merge'); setPreview(selected); }
    });
  }

  function discard() {
    if (busy || !preview) return;
    void invoke('discard_restore', { id: preview.id }).catch(() => undefined);
    setPreview(undefined);
    setError('');
  }

  return <details className="data-settings" onToggle={(event) => setOpen(event.currentTarget.open)}>
    <summary>Data and backups</summary>
    {!status && !error && <p className="task-hint" role="status">Loading backups…</p>}
    {status && <>
      <dl className="data-paths">
        <div><dt>Database</dt><dd>{status.database}</dd></div>
        <div><dt>Backup folder</dt><dd>{status.directory}</dd></div>
      </dl>
      <div className="task-actions data-actions">
        <button type="button" disabled={busy} onClick={() => saveDirectory(false)}>Choose folder</button>
        {status.customDirectory && <button type="button" disabled={busy} onClick={() => saveDirectory(true)}>Use default folder</button>}
        <button type="button" disabled={busy} onClick={() => void perform(async () => {
          setStatus(await invoke<BackupStatus>('create_backup'));
          setMessage('Backup saved.');
        })}>Back up now</button>
        <button type="button" disabled={busy} onClick={() => void perform(async () => {
          if (await invoke<boolean>('export_backup')) setMessage('Backup exported.');
        })}>Export backup</button>
        <button type="button" disabled={busy} onClick={() => review(null)}>Import backup…</button>
      </div>
      {status.backups.length ? <ul className="data-backups" aria-label="Available backups">
        {status.backups.map((file) => <li key={file.name}>
          <span>{timestamp.format(file.modifiedAt * 1000)} <small>{sizeLabel(file.size)}</small></span>
          <button type="button" disabled={busy} aria-label={`Restore backup ${timestamp.format(file.modifiedAt * 1000)}`} onClick={() => review(file.name)}>Restore…</button>
        </li>)}
      </ul> : <p className="task-hint">No backups in this folder.</p>}
    </>}
    {message && <p className="task-hint" role="status">{message}</p>}
    {error && !preview && <p className="status" role="alert">{error}</p>}
    {preview && <Modal title="Restore backup" onClose={discard}>
      <div className="task-form">
        <p className="task-hint">{preview.name}</p>
        <label>Restore mode<select value={mode} disabled={busy} onChange={(event) => setMode(event.target.value as Mode)}>
          <option value="merge">Merge</option><option value="replace">Replace</option>
        </select></label>
        <p className="task-hint">{mode === 'merge'
          ? `Keep current data and add ${taskCount(preview.mergeAdditions)} from the backup.`
          : `Replace your current data (${taskCount(preview.currentTasks)}) with this backup (${taskCount(preview.backupTasks)}), including its window and reminder settings.`}
          {' '}Kairos saves a backup of your current data first.
        </p>
        {error && <p className="status" role="alert">{error}</p>}
        <div className="task-actions">
          <button type="button" disabled={busy} onClick={discard}>Cancel</button>
          <button type="button" disabled={busy} className={mode === 'replace' ? 'task-danger' : 'task-primary'} onClick={() => void perform(async () => {
            await invoke<BackupStatus>('restore_backup', { id: preview.id, mode });
            window.location.reload();
          })}>{busy ? 'Restoring…' : mode === 'merge' ? 'Merge backup' : 'Replace data'}</button>
        </div>
      </div>
    </Modal>}
  </details>;
}
