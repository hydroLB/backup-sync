import { Button } from '../../ui/Button';
import { StateBlock } from '../../ui/StateBlock';

type SetupNoticeStep = 'destination' | 'folders' | 'ready';

type Props = {
  step: SetupNoticeStep;
  destinationCount: number;
  watchedCount: number;
  busy: boolean;
  onChooseDestination: () => void;
  onAddPath: () => void;
};

/**
 * Summary: Render the single highest-priority setup cue for the minimal shell.
 *
 * Inputs: Setup state, current counts, busy flag, and quick-action handlers.
 *
 * Outputs: A focused setup notice block.
 *
 * Side effects: Calls the provided quick action when the user follows the prompt.
 *
 * Error handling: None.
 *
 * Ties to other methods: Used by `MinimalMain` before the setup cards.
 *
 * Why this exists: Keep the next required setup step obvious without repeating helper copy in every card.
 */
export function SetupNotice({
  step,
  destinationCount,
  watchedCount,
  busy,
  onChooseDestination,
  onAddPath,
}: Props) {
  if (step === 'ready') {
    return (
      <StateBlock
        tone="info"
        title="Setup complete"
        message={`Watching ${watchedCount} protected path${watchedCount === 1 ? '' : 's'} across ${destinationCount} destination${destinationCount === 1 ? '' : 's'}. Restore unlocks after backup history is written.`}
        className="setup-notice"
      />
    );
  }

  if (step === 'destination') {
    return (
      <StateBlock
        tone="info"
        title="Choose a primary destination first"
        message="Backup setup starts here. Protected paths and restore stay inactive until a destination is selected."
        className="setup-notice"
        action={
          <Button type="button" size="sm" onClick={onChooseDestination} disabled={busy}>
            Choose destination
          </Button>
        }
      />
    );
  }

  return (
    <StateBlock
      tone="info"
      title="Add the first protected path"
      message="New protected paths are copied to every configured destination. Restore becomes useful after the first saved version exists."
      className="setup-notice"
      action={
        <Button type="button" size="sm" onClick={onAddPath} disabled={busy}>
          Add path…
        </Button>
      }
    />
  );
}
