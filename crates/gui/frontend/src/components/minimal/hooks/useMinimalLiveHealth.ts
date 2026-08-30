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

/** Avoid repeatedly firing toasts when issue sets are unchanged. */
function issueKey(issues: string[]): string {
  return issues.slice().sort().join('\n');
}

/** Keep destination health issues visible without flooding the interface. */
function destinationWarningFromIssues(issues: string[]): string | null {
  if (issues.length === 0) return null;
  if (issues.length === 1) {
    return `Destination issue: ${issues[0]!}`;
  }
  return `Destination issues (${issues.length}): ${issues.slice(0, 2).join(' • ')}`;
}

/** Keep watched source health actionable and compact. */
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

/** Surface filesystem regressions quickly without requiring manual checks. */
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
      if (typeof document !== 'undefined' && document.hidden) return;
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
        const access = await testAccess();
        if (cancelled) return;
        const nextMissing = access.watched_missing ?? [];
        const nextInaccessible = access.watched_unwritable ?? [];
        const nextDestinationKey = issueKey(nextDestinationIssues);
        const nextWatchedKey = issueKey([...nextMissing, ...nextInaccessible]);

        if (prevDestinationKeyRef.current !== nextDestinationKey) {
          setDestinationIssues(nextDestinationIssues);
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
          setWatchedMissing(nextMissing);
          setWatchedInaccessible(nextInaccessible);
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
    const onVisibilityChange = () => {
      if (!document.hidden) void refresh();
    };
    document.addEventListener('visibilitychange', onVisibilityChange);
    return () => {
      cancelled = true;
      clearTimeout(kickoff);
      clearInterval(id);
      document.removeEventListener('visibilitychange', onVisibilityChange);
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
