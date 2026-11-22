import { useState } from 'react';
import { saveDailyGreeting } from './desktop';
import type { DailyGreeting } from './desktop';

export function GreetingSettings({ greeting, onSave }: {
  greeting: string; onSave: (greeting: DailyGreeting | null) => void;
}) {
  const [draft, setDraft] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const text = draft ?? greeting;

  async function save(text: string) {
    setSaving(true);
    setError('');
    try {
      const next = await saveDailyGreeting(text);
      onSave(next);
      setDraft(null);
    } catch (reason) {
      setError(typeof reason === 'string' ? reason : 'Could not save today’s greeting.');
    } finally {
      setSaving(false);
    }
  }

  return <form className="task-form daily-greeting-form" onSubmit={(event) => { event.preventDefault(); void save(text); }}>
    <label>Today’s greeting<input type="text" value={text} disabled={saving} aria-describedby="greeting-expiration"
      onChange={(event) => setDraft(event.target.value)} /></label>
    <p id="greeting-expiration" className="task-hint">Returns to the default at local midnight.</p>
    <div className="task-actions">
      <button type="submit" disabled={saving || text.trim() === greeting}>Save greeting</button>
      {greeting && <button type="button" disabled={saving} onClick={() => void save('')}>Use default</button>}
    </div>
    {error && <p className="status" role="alert">{error}</p>}
  </form>;
}
