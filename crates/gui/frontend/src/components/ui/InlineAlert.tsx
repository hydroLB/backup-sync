import { ReactNode } from 'react';

type AlertKind = 'error' | 'warn' | 'info' | 'success';

type Props = {
  kind: AlertKind;
  children: ReactNode;
  className?: string;
  dragRegion?: boolean;
};

/** Replace repeated one-off warning colors and spacing. */
export function InlineAlert({ kind, children, className, dragRegion = false }: Props) {
  const role = kind === 'error' ? 'alert' : 'status';
  const tone = `alert alert-${kind}`;
  return (
    <div
      className={className ? `${tone} ${className}` : tone}
      role={role}
      {...(dragRegion ? { 'data-tauri-drag-region': '' } : {})}
    >
      {children}
    </div>
  );
}
