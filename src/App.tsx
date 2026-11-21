import { useEffect, useState } from 'react';
import { getStorageInfo, isDesktop } from './desktop';

type StorageState = { status: 'loading' | 'ready' } | { status: 'error'; message: string };

const dateFormat = new Intl.DateTimeFormat(undefined, {
  weekday: 'long', month: 'long', day: 'numeric',
});

export function App() {
  const [storage, setStorage] = useState<StorageState>({ status: 'loading' });
  const [attempt, setAttempt] = useState(0);
  const [today, setToday] = useState(() => dateFormat.format(new Date()));

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

  return (
    <main className="panel">
      <header className="panel-header">
        <h1>Kairos</h1>
        <time>{today}</time>
      </header>
      <div className="panel-body">
        <p className="tagline">At your own pace.</p>
        {!isDesktop && <p className="status">Open the desktop app to access local storage.</p>}
        {isDesktop && storage.status === 'loading' && <p className="status" role="status">Opening local storage…</p>}
        {isDesktop && storage.status === 'ready' && <p className="status" role="status">Local storage is ready.</p>}
        {isDesktop && storage.status === 'error' && (
          <div className="storage-error">
            <p className="status" role="alert">{storage.message}</p>
            <button type="button" onClick={retryStorage}>Try again</button>
          </div>
        )}
      </div>
    </main>
  );
}
