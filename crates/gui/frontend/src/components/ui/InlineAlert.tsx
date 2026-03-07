import { ReactNode } from 'react';

type AlertKind = 'error' | 'warn' | 'info' | 'success';

type Props = {
  kind: AlertKind;
  children: ReactNode;
  className?: string;
};

/**
 * Summary: Render a compact inline alert banner for contextual guidance and warnings.
 *
 * Inputs: Alert severity, children content, and optional class name overrides.
 *
 * Outputs: Inline alert element tree.
 *
 * Side effects: None.
 *
 * Error handling: None.
 *
 * Ties to other methods: Used across cards and modals to standardize message styling.
 *
 * Why this exists: Replace repeated one-off warning colors and spacing.
 */
export function InlineAlert({ kind, children, className }: Props) {
  const role = kind === 'error' ? 'alert' : 'status';
  const tone = `alert alert-${kind}`;
  return (
    <div className={className ? `${tone} ${className}` : tone} role={role}>
      {children}
    </div>
  );
}
