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

/** Keep form structure consistent and easy to restyle from global CSS. */
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
  const labelContent = (
    <>
      <span>{label}</span>
      {required && (
        <span className="form-field__required" aria-hidden="true">
          *
        </span>
      )}
    </>
  );

  return (
    <div className={classes}>
      {htmlFor ? (
        <label className="form-field__label" htmlFor={htmlFor}>
          {labelContent}
        </label>
      ) : (
        <div className="form-field__label">{labelContent}</div>
      )}
      {children}
      {hint && <div className="form-field__hint">{hint}</div>}
      {error && <div className="form-field__error">{error}</div>}
    </div>
  );
}
