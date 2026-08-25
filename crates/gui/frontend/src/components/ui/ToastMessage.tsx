import { useMemo } from 'react';

type ToastKind = 'ok' | 'error' | 'info';

type ToastPayload = {
  msg: string;
  kind: ToastKind;
};

type Props = {
  toast: ToastPayload | null;
  onDismiss?: () => void;
};

/** Keep transient feedback visuals and ARIA behavior consistent. */
export function ToastMessage({ toast, onDismiss }: Props) {
  const className = useMemo(() => {
    if (!toast) return '';
    if (toast.kind === 'error') return 'toast toast-error';
    if (toast.kind === 'ok') return 'toast toast-ok';
    return 'toast toast-info';
  }, [toast]);

  if (!toast) return null;

  return (
    <div
      className={className}
      role={toast.kind === 'error' ? 'alert' : 'status'}
      aria-live={toast.kind === 'error' ? 'assertive' : 'polite'}
    >
      <span className="toast-text">{toast.msg}</span>
      {onDismiss && (
        <button
          type="button"
          className="toast-dismiss"
          onClick={onDismiss}
          aria-label="Dismiss message"
        >
          Close
        </button>
      )}
    </div>
  );
}
