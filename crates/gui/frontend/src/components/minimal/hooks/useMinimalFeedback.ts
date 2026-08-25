import { useCallback, useEffect, useRef, useState } from 'react';
import { UI_TUNING } from '../../../config/uiTuning';

type EventKind = 'ok' | 'error' | 'info';
export type SavePulseScope = 'none' | 'global' | 'header' | 'destination' | 'folders' | 'restore';

type ToastPayload = {
  msg: string;
  kind: EventKind;
} | null;

type InlinePayload = {
  msg: string;
  kind: 'success' | 'error';
} | null;

type Params = {
  onEvent: (msg: string, kind?: EventKind) => void;
};

type MinimalFeedbackState = {
  toast: ToastPayload;
  inline: InlinePayload;
  savePulseActive: boolean;
  savePulseScope: SavePulseScope;
  pausePulseActive: boolean;
  showSavedBadge: boolean;
  savedBadgeNonce: number;
  emitEvent: (msg: string, kind?: EventKind, savePulseScope?: SavePulseScope) => void;
  clearToast: () => void;
  clearInline: () => void;
};

/** Ensure success/error/info messages are always visible in minimal mode. */
export function useMinimalFeedback({ onEvent }: Params): MinimalFeedbackState {
  const [toast, setToast] = useState<ToastPayload>(null);
  const [inline, setInline] = useState<InlinePayload>(null);
  const [savePulseActive, setSavePulseActive] = useState(false);
  const [savePulseScope, setSavePulseScope] = useState<SavePulseScope>('none');
  const [pausePulseActive, setPausePulseActive] = useState(false);
  const [showSavedBadge, setShowSavedBadge] = useState(false);
  const [savedBadgeNonce, setSavedBadgeNonce] = useState(0);
  const lastSavedBadgeTriggerRef = useRef<{ ts: number; tag: string } | null>(null);
  const savedBadgeHideTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const savedBadgeVisibleRef = useRef(false);

  // Prevent accidental double-triggers from duplicate events.
  // Intentionally short so deliberate repeated actions can retrigger the badge.
  const sameTagRetriggerMinMs = 90;

  /** Prevent duplicate Saved flashes while still providing crisp feedback. */
  const triggerSavedBadge = useCallback((tag: string) => {
    try {
      const now = Date.now();
      const last = lastSavedBadgeTriggerRef.current;
      // Guard against duplicate triggers firing in quick succession.
      if (last && last.tag === tag && now - last.ts < sameTagRetriggerMinMs) {
        return;
      }
      lastSavedBadgeTriggerRef.current = { ts: now, tag };
      savedBadgeVisibleRef.current = true;
      setShowSavedBadge(true);
      // Always restart the animation on a new trigger so rapid successive actions
      // (for example, spam start/pause) still show a visible confirmation.
      setSavedBadgeNonce((prev) => prev + 1);
      if (savedBadgeHideTimerRef.current) {
        clearTimeout(savedBadgeHideTimerRef.current);
      }
      savedBadgeHideTimerRef.current = setTimeout(() => {
        savedBadgeVisibleRef.current = false;
        setShowSavedBadge(false);
      }, 1500);
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      console.warn(
        `[useMinimalFeedback::triggerSavedBadge] Failed to animate saved badge: ${reason}`,
      );
      savedBadgeVisibleRef.current = true;
      setShowSavedBadge(true);
    }
  }, []);

  const emitEvent = useCallback(
    (msg: string, kind: EventKind = 'info', pulseScope: SavePulseScope = 'none') => {
      const isPauseEvent = kind === 'ok' && /\bpaused\b/i.test(msg);
      const isSuccessEvent = kind === 'ok';
      const isSavedEvent = isSuccessEvent && /^saved\.$/i.test(msg.trim());
      const isRunningEvent = kind === 'ok' && /\brunning\b/i.test(msg);
      const shouldShowSavedBadge = isPauseEvent || pulseScope !== 'none' || isSavedEvent;
      if (isPauseEvent) {
        setToast(null);
        setSavePulseActive(false);
        setSavePulseScope('none');
        // Pausing is still a successful state change; show the same confirmation as saves.
        if (shouldShowSavedBadge) {
          triggerSavedBadge('pause');
        }
        setPausePulseActive(true);
      } else if (isSuccessEvent) {
        setPausePulseActive(false);
        setSavePulseScope(pulseScope);
        setSavePulseActive(pulseScope !== 'none');
        if (!isSavedEvent && !isRunningEvent && pulseScope !== 'none') {
          setInline({ msg, kind: 'success' });
        }
        // Only show Saved for user-driven operations. Background "ok" transitions
        // (for example, destination health recovered) should not surface as "Saved".
        if (shouldShowSavedBadge) {
          triggerSavedBadge(isSavedEvent ? 'saved' : `scope:${pulseScope}`);
        }
      } else {
        setSavePulseScope('none');
        setToast({ msg, kind });
      }
      if (kind === 'error') {
        setInline({ msg, kind: 'error' });
      }
      onEvent(msg, kind);
    },
    [onEvent, triggerSavedBadge],
  );

  useEffect(() => {
    if (!toast) return;
    const id = setTimeout(() => setToast(null), UI_TUNING.toastDismissMs);
    return () => clearTimeout(id);
  }, [toast]);

  useEffect(() => {
    if (!inline) return;
    const id = setTimeout(() => setInline(null), UI_TUNING.toastDismissMs * 2);
    return () => clearTimeout(id);
  }, [inline]);

  useEffect(() => {
    if (!savePulseActive) return;
    const id = setTimeout(() => setSavePulseActive(false), 850);
    return () => clearTimeout(id);
  }, [savePulseActive]);

  useEffect(() => {
    if (!pausePulseActive) return;
    const id = setTimeout(() => setPausePulseActive(false), 900);
    return () => clearTimeout(id);
  }, [pausePulseActive]);

  useEffect(() => {
    return () => {
      if (savedBadgeHideTimerRef.current) {
        clearTimeout(savedBadgeHideTimerRef.current);
        savedBadgeHideTimerRef.current = null;
      }
    };
  }, []);

  return {
    toast,
    inline,
    savePulseActive,
    savePulseScope,
    pausePulseActive,
    showSavedBadge,
    savedBadgeNonce,
    emitEvent,
    clearToast: () => setToast(null),
    clearInline: () => setInline(null),
  };
}
