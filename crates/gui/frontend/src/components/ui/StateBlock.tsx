import { ReactNode } from 'react';

type StateTone = 'loading' | 'empty' | 'error' | 'success' | 'info';

type Props = {
  tone: StateTone;
  title: string;
  message?: string;
  action?: ReactNode;
  className?: string;
};

/** Standardize feedback states and remove one-off loading and empty placeholders. */
export function StateBlock({ tone, title, message, action, className }: Props) {
  const classes = ['state-block', `state-${tone}`, className ?? ''].filter(Boolean).join(' ');
  const role = tone === 'error' ? 'alert' : 'status';

  return (
    <div className={classes} role={role}>
      {tone === 'loading' && <span className="state-block__spinner" aria-hidden="true" />}
      <div className="state-block__content">
        <div className="state-block__title">{title}</div>
        {message && <div className="state-block__message">{message}</div>}
      </div>
      {action && <div className="state-block__action">{action}</div>}
    </div>
  );
}
