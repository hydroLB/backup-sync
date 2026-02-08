import { ReactNode } from 'react';

type Props = {
  label: string;
  htmlFor?: string;
  hint?: string;
  error?: string | null;
  required?: boolean;
  className?: string;
  children: ReactNode;
};

/**
 * Summary: Render a standardized form field wrapper with label, hint, and inline error support.
 *
 * Inputs: Label text, optional field id mapping, optional hint/error text, required flag, optional class override, and field control content.
 * Outputs: A form-field element tree.
 * Side effects: None.
 * Error handling: None.
 * Ties to other methods: Used by modal and settings forms to align spacing and validation presentation.
 * Why this exists: Keep form structure consistent and easy to restyle from global CSS.
 */
export function FormField({
  label,
  htmlFor,
  hint,
  error,
  required = false,
  className,
  children,
}: Props) {
  const classes = ['form-field', error ? 'has-error' : '', className ?? '']
    .filter(Boolean)
    .join(' ');

  return (
    <div className={classes}>
      <label className="form-field__label" htmlFor={htmlFor}>
        <span>{label}</span>
        {required && (
          <span className="form-field__required" aria-hidden="true">
            *
          </span>
        )}
      </label>
      {children}
      {hint && <div className="form-field__hint">{hint}</div>}
      {error && <div className="form-field__error">{error}</div>}
    </div>
  );
}
