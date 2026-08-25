import type { ReactNode } from 'react';
import { Button } from '../../ui/Button';

type SetupNoticeStep = 'destination' | 'folders' | 'ready';

type Props = {
  step: SetupNoticeStep;
  destinationCount: number;
  watchedCount: number;
  busy: boolean;
  onChooseDestination: () => void;
  onAddPath: () => void;
};

type GuideStepProps = {
  number: string;
  label: string;
  title: string;
  description: string;
  state: 'complete' | 'current' | 'locked';
  status: string;
  action?: ReactNode;
};

function GuideStep({ number, label, title, description, state, status, action }: GuideStepProps) {
  return (
    <li className={`workflow-step workflow-step--${state}`}>
      <div className="workflow-step__topline">
        <span className="workflow-step__number" aria-hidden="true">
          {number}
        </span>
        <span className="workflow-step__label">{label}</span>
        <span className="workflow-step__status">{status}</span>
      </div>
      <h3>{title}</h3>
      <p>{description}</p>
      {action && <div className="workflow-step__action">{action}</div>}
    </li>
  );
}

/** Explain the complete workflow in place while highlighting only the next useful action. */
export function SetupNotice({
  step,
  destinationCount,
  watchedCount,
  busy,
  onChooseDestination,
  onAddPath,
}: Props) {
  const destinationReady = destinationCount > 0;
  const foldersReady = watchedCount > 0;
  const ready = step === 'ready';

  const title =
    step === 'destination'
      ? 'Start with a safe place for your copies.'
      : step === 'folders'
        ? 'Storage is ready. Choose what matters.'
        : 'Your protection workflow is connected.';

  const message =
    step === 'destination'
      ? 'Backup Sync needs a destination before it can preserve any versions.'
      : step === 'folders'
        ? 'Add a folder or file and Backup Sync will begin building recoverable history.'
        : 'New changes are saved automatically, and Restore lets you return to an earlier version.';

  return (
    <section className="workflow-guide" aria-labelledby="workflow-guide-title">
      <div className="workflow-guide__intro">
        <div>
          <span className="workflow-guide__eyebrow">How Backup Sync works</span>
          <h2 id="workflow-guide-title">{title}</h2>
        </div>
        <p>{message}</p>
      </div>

      <ol className="workflow-steps">
        <GuideStep
          number="01"
          label="Storage"
          title="Choose where copies live"
          description="Use one destination, or add a second independent location for automatic mirroring."
          state={destinationReady ? 'complete' : 'current'}
          status={destinationReady ? `${destinationCount} ready` : 'Start here'}
          action={
            step === 'destination' ? (
              <Button type="button" size="sm" onClick={onChooseDestination} disabled={busy}>
                Choose destination
              </Button>
            ) : undefined
          }
        />
        <GuideStep
          number="02"
          label="Protection"
          title="Add what matters"
          description="Select folders or files. Only changed content is stored when a new version is created."
          state={foldersReady ? 'complete' : destinationReady ? 'current' : 'locked'}
          status={
            foldersReady ? `${watchedCount} protected` : destinationReady ? 'Next' : 'Waiting'
          }
          action={
            step === 'folders' ? (
              <Button type="button" size="sm" onClick={onAddPath} disabled={busy}>
                Add protected path
              </Button>
            ) : undefined
          }
        />
        <GuideStep
          number="03"
          label="Recovery"
          title="Return to any version"
          description="Browse saved versions, recover one file, or restore an entire protected folder."
          state={ready ? 'complete' : 'locked'}
          status={ready ? 'Available' : 'Waiting'}
        />
      </ol>
    </section>
  );
}
