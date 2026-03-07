import { ButtonHTMLAttributes, ReactNode } from 'react';

type ButtonTone = 'primary' | 'secondary' | 'danger';
type ButtonSize = 'md' | 'sm';

type Props = ButtonHTMLAttributes<HTMLButtonElement> & {
  tone?: ButtonTone;
  size?: ButtonSize;
  block?: boolean;
  loading?: boolean;
  loadingLabel?: string;
  leading?: ReactNode;
  trailing?: ReactNode;
};

/**
 * Summary: Render a consistent button surface with shared variants and loading behavior.
 *
 * Inputs: Native button attributes, tone/size options, optional loading state, and optional leading/trailing content.
 *
 * Outputs: A button element with normalized classes, disabled handling, and optional spinner content.
 *
 * Side effects: None.
 *
 * Error handling: None.
 *
 * Ties to other methods: Used by minimal, status, and settings views to centralize interaction styling.
 *
 * Why this exists: Reduce repeated button markup and keep global restyling changes in one place.
 */
export function Button({
  tone = 'primary',
  size = 'md',
  block = false,
  loading = false,
  loadingLabel,
  leading,
  trailing,
  className,
  disabled,
  children,
  ...props
}: Props) {
  const classes = [
    'btn',
    tone === 'secondary' ? 'secondary' : '',
    tone === 'danger' ? 'btn-danger' : '',
    size === 'sm' ? 'btn-sm' : '',
    block ? 'btn-block' : '',
    loading ? 'btn-loading' : '',
    className ?? '',
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <button className={classes} disabled={disabled || loading} aria-busy={loading} {...props}>
      {loading ? (
        <>
          <span className="btn-spinner" aria-hidden="true" />
          <span className="btn-content">{loadingLabel ?? children}</span>
        </>
      ) : (
        <>
          {leading}
          <span className="btn-content">{children}</span>
          {trailing}
        </>
      )}
    </button>
  );
}
