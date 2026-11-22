import { useEffect, useRef } from 'react';
import type { ReactNode } from 'react';

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
