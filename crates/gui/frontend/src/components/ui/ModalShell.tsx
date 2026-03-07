import { ReactNode, useEffect, useId, useRef } from 'react';
import { Button } from './Button';

type Props = {
  open: boolean;
  title: string;
  onClose: () => void;
  busy?: boolean;
  description?: string;
  children: ReactNode;
  footer?: ReactNode;
  closeLabel?: string;
  closeAriaLabel?: string;
  closeButtonClassName?: string;
};

const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

/**
 * Summary: Collect focusable elements inside the modal, preserving tab order.
 *
 * Inputs: Root modal element.
 *
 * Outputs: Array of visible focusable HTMLElements.
 *
 * Side effects: None.
 *
 * Error handling: None.
 *
 * Ties to other methods: Used by `ModalShell` focus trap and initial-focus behavior.
 *
 * Why this exists: Keep keyboard navigation bounded to the active modal dialog.
 */
function focusableElements(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter((element) => {
    if (element.tabIndex < 0) return false;
    const rects = element.getClientRects();
    return rects.length > 0;
  });
}

/**
 * Summary: Render a reusable accessible modal shell with overlay and keyboard dismissal.
 *
 * Inputs: Open state, title, close handler, optional busy flag, optional description, and body/footer content.
 *
 * Outputs: Modal element tree when open.
 *
 * Side effects: Registers and removes Escape-key listeners while the dialog is open.
 *
 * Error handling: None.
 *
 * Ties to other methods: Used by minimal restore and hardening dialogs.
 *
 * Why this exists: Centralize dialog semantics and interaction behavior across screens.
 */
export function ModalShell({
  open,
  title,
  onClose,
  busy = false,
  description,
  children,
  footer,
  closeLabel = 'Close',
  closeAriaLabel,
  closeButtonClassName,
}: Props) {
  const titleId = useId();
  const descriptionId = useId();
  const dialogRef = useRef<HTMLDivElement | null>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);
  const busyRef = useRef<boolean>(busy);
  const onCloseRef = useRef<() => void>(onClose);

  useEffect(() => {
    busyRef.current = busy;
  }, [busy]);

  useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);

  useEffect(() => {
    if (!open) return;
    const dialog = dialogRef.current;
    if (!dialog) return;

    previousFocusRef.current =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;

    const initialTargets = focusableElements(dialog);
    if (initialTargets.length > 0) {
      initialTargets[0]!.focus();
    } else {
      dialog.focus();
    }

    /**
     * Summary: handleKeyDown orchestrates this method's core behavior.
     *
     * Inputs: Method parameters and required receiver state.
     *
     * Outputs: Return value and observable result for callers.
     *
     * Side effects: None beyond this method's explicit operations.
     *
     * Error handling: Propagates contextual errors to the caller when operations fail.
     *
     * Ties to other methods: Invoked by and composes with adjacent module methods.
     *
     * Why this exists: Keeps this behavior isolated, testable, and reusable.
     */

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      if (busyRef.current) return;
      onCloseRef.current();
    };

    /**
     * Summary: handleTabTrap orchestrates this method's core behavior.
     *
     * Inputs: Method parameters and required receiver state.
     *
     * Outputs: Return value and observable result for callers.
     *
     * Side effects: None beyond this method's explicit operations.
     *
     * Error handling: Propagates contextual errors to the caller when operations fail.
     *
     * Ties to other methods: Invoked by and composes with adjacent module methods.
     *
     * Why this exists: Keeps this behavior isolated, testable, and reusable.
     */

    const handleTabTrap = (event: KeyboardEvent) => {
      if (event.key !== 'Tab') return;
      const targets = focusableElements(dialog);
      if (targets.length === 0) {
        event.preventDefault();
        dialog.focus();
        return;
      }
      const first = targets[0]!;
      const last = targets[targets.length - 1]!;
      const active = document.activeElement;
      if (event.shiftKey) {
        if (active === first || !dialog.contains(active)) {
          event.preventDefault();
          last.focus();
        }
        return;
      }
      if (active === last) {
        event.preventDefault();
        first.focus();
      }
    };

    dialog.addEventListener('keydown', handleKeyDown);
    dialog.addEventListener('keydown', handleTabTrap);
    return () => {
      dialog.removeEventListener('keydown', handleKeyDown);
      dialog.removeEventListener('keydown', handleTabTrap);
      previousFocusRef.current?.focus();
    };
  }, [open]);

  if (!open) return null;

  return (
    <div
      className="modal-overlay"
      onMouseDown={(event) => {
        if (event.target !== event.currentTarget) return;
        if (busy) return;
        onClose();
      }}
    >
      <div
        ref={dialogRef}
        className="card modal-card"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={description ? descriptionId : undefined}
        tabIndex={-1}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="modal-header">
          <h2 id={titleId} className="section-heading">
            {title}
          </h2>
          <Button
            type="button"
            tone="secondary"
            size="sm"
            className={closeButtonClassName}
            aria-label={closeAriaLabel ?? `Close ${title}`}
            onClick={onClose}
            disabled={busy}
          >
            {closeLabel}
          </Button>
        </div>
        {description && (
          <p id={descriptionId} className="modal-help">
            {description}
          </p>
        )}
        {children}
        {footer && <div className="modal-footer">{footer}</div>}
      </div>
    </div>
  );
}
