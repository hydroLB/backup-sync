import { useEffect, useMemo, useRef, useState } from 'react';
import { UI_TUNING } from '../../../config/uiTuning';
import { checkDestination, testAccess } from '../../../services/system';
import { Config } from '../../../domain/config';

type EventKind = 'ok' | 'error' | 'info';

type Params = {
  cfg: Config | null;
  onEvent: (msg: string, kind?: EventKind) => void;
  suspend?: boolean;
};

type LiveHealthState = {
  destinationHealthWarning: string | null;
  watchedHealthWarning: string | null;
};

/**
 * Summary: Build a stable key for destination and watched issue comparisons.
 *
 * Inputs: List of issue strings.
 *
 * Outputs: Deterministic key string for transition detection.
 *
 * Side effects: None.
 *
 * Error handling: None.
 *
 * Ties to other methods: Used by `useMinimalLiveHealth` event transition logic.
 *
 * Why this exists: Avoid repeatedly firing toasts when issue sets are unchanged.
 */
function issueKey(issues: string[]): string {
  return issues.slice().sort().join('\n');
}

/**
 * Summary: Build a concise destination warning string for inline UI display.
 *
 * Inputs: Destination issue list.
 *
 * Outputs: User-facing warning string or null.
 *
 * Side effects: None.
 *
 * Error handling: None.
 *
 * Ties to other methods: Used by `useMinimalLiveHealth`.
 *
 * Why this exists: Keep destination health issues visible without flooding the interface.
 */
function destinationWarningFromIssues(issues: string[]): string | null {
  if (issues.length === 0) return null;
  if (issues.length === 1) {
    return `Destination issue: ${issues[0]!}`;
  }
  return `Destination issues (${issues.length}): ${issues.slice(0, 2).join(' • ')}`;
}

/**
 * Summary: Build a concise watched path warning string for inline UI display.
 *
 * Inputs: Missing and inaccessible watched path lists.
 *
 * Outputs: User-facing warning string or null.
 *
 * Side effects: None.
 *
 * Error handling: None.
 *
 * Ties to other methods: Used by `useMinimalLiveHealth`.
 *
 * Why this exists: Keep watched source health actionable and compact.
 */
function watchedWarningFromIssues(missing: string[], inaccessible: string[]): string | null {
  const total = missing.length + inaccessible.length;
  if (total === 0) return null;
  const parts: string[] = [];
  if (missing.length > 0) {
    parts.push(`${missing.length} missing`);
  }
  if (inaccessible.length > 0) {
    parts.push(`${inaccessible.length} inaccessible`);
  }
  return `Protected path issues (${total}): ${parts.join(', ')}.`;
}

/**
 * Summary: Poll destination and watched-path accessibility for minimal mode.
 *
 * Inputs: Config and event callback.
 *
 * Outputs: Inline warning strings for destination and watched path health.
 *
 * Side effects: Calls IPC probes on a timer and emits transition toasts when health changes.
 *
 * Error handling: Tolerates transient probe failures and emits actionable probe errors.
 *
 * Ties to other methods: Consumed by `MinimalMain` to render live health warnings.
 *
 * Why this exists: Surface filesystem regressions quickly without requiring manual checks.
 */
export function useMinimalLiveHealth({ cfg, onEvent, suspend = false }: Params): LiveHealthState {
  const [destinationIssues, setDestinationIssues] = useState<string[]>([]);
  const [watchedMissing, setWatchedMissing] = useState<string[]>([]);
  const [watchedInaccessible, setWatchedInaccessible] = useState<string[]>([]);
  const prevDestinationKeyRef = useRef<string>('');
  const prevWatchedKeyRef = useRef<string>('');
  const inFlightRef = useRef(false);

  useEffect(() => {
    if (!cfg) {
      setDestinationIssues([]);
      setWatchedMissing([]);
      setWatchedInaccessible([]);
      prevDestinationKeyRef.current = '';
      prevWatchedKeyRef.current = '';
      return;
    }
    let cancelled = false;

    const refresh = async () => {
      if (suspend) return;
      if (inFlightRef.current) return;
      inFlightRef.current = true;
      try {
        const destinationChecks = await Promise.all(
          (cfg.destinations || []).map(async (destination) => {
            const result = await checkDestination(destination.path);
            if (result.writable) return null;
            const name =
              (destination.label && destination.label.trim().length > 0
                ? destination.label.trim()
                : destination.path) || destination.id;
            return `${name}: ${result.message}`;
          }),
        );
        if (cancelled) return;
        const nextDestinationIssues = destinationChecks.filter((entry): entry is string => !!entry);
        setDestinationIssues(nextDestinationIssues);

        const access = await testAccess();
        if (cancelled) return;
        const nextMissing = access.watched_missing ?? [];
        const nextInaccessible = access.watched_unwritable ?? [];
        setWatchedMissing(nextMissing);
        setWatchedInaccessible(nextInaccessible);

        const nextDestinationKey = issueKey(nextDestinationIssues);
        const nextWatchedKey = issueKey([...nextMissing, ...nextInaccessible]);

        if (prevDestinationKeyRef.current !== nextDestinationKey) {
          if (prevDestinationKeyRef.current.length === 0 && nextDestinationIssues.length > 0) {
            onEvent(
              destinationWarningFromIssues(nextDestinationIssues) ?? 'Destination issue detected.',
              'error',
            );
          } else if (
            prevDestinationKeyRef.current.length > 0 &&
            nextDestinationIssues.length === 0
          ) {
            onEvent('All destinations are reachable again.', 'ok');
          } else if (nextDestinationIssues.length > 0) {
            onEvent(
              destinationWarningFromIssues(nextDestinationIssues) ?? 'Destination issue updated.',
              'error',
            );
          }
          prevDestinationKeyRef.current = nextDestinationKey;
        }

        if (prevWatchedKeyRef.current !== nextWatchedKey) {
          if (prevWatchedKeyRef.current.length === 0 && nextWatchedKey.length > 0) {
            onEvent(
              watchedWarningFromIssues(nextMissing, nextInaccessible) ??
                'Protected path issue detected.',
              'error',
            );
          } else if (prevWatchedKeyRef.current.length > 0 && nextWatchedKey.length === 0) {
            onEvent('All protected paths are accessible again.', 'ok');
          } else if (nextWatchedKey.length > 0) {
            onEvent(
              watchedWarningFromIssues(nextMissing, nextInaccessible) ??
                'Protected path issue updated.',
              'error',
            );
          }
          prevWatchedKeyRef.current = nextWatchedKey;
        }
      } catch (error) {
        if (cancelled) return;
        const reason = error instanceof Error ? error.message : String(error);
        onEvent(`Live health check failed: ${reason}`, 'error');
      } finally {
        inFlightRef.current = false;
      }
    };

    // Delay the first probe slightly so it does not compete with the user's
    // first interaction (for example, opening a picker dialog).
    const kickoff = setTimeout(() => {
      void refresh();
    }, 900);
    const id = setInterval(() => {
      void refresh();
    }, UI_TUNING.liveHealthRefreshMs);
    return () => {
      cancelled = true;
      clearTimeout(kickoff);
      clearInterval(id);
    };
  }, [cfg, onEvent, suspend]);

  const destinationHealthWarning = useMemo(
    () => destinationWarningFromIssues(destinationIssues),
    [destinationIssues],
  );
  const watchedHealthWarning = useMemo(
    () => watchedWarningFromIssues(watchedMissing, watchedInaccessible),
    [watchedMissing, watchedInaccessible],
  );

  return { destinationHealthWarning, watchedHealthWarning };
}
