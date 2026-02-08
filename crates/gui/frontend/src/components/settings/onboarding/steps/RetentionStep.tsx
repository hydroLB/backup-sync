type Props = {
  retention: number;
  onChangeRetention: (value: number) => void;
};

/**
 * Summary: Render onboarding step 3 (choose how many versions to keep).
 *
 * Inputs: Current retention value and change handler.
 * Outputs: A step body element tree.
 * Side effects: Calls `onChangeRetention` on slider changes.
 * Error handling: Delegated to the provided handler.
 * Ties to other methods: Used by `OnboardingOverlay` body rendering.
 * Why this exists: Keep the retention slider UI isolated and easy to reuse.
 */
export function RetentionStep({ retention, onChangeRetention }: Props) {
  return (
    <>
      <p className="muted">How many old versions do you want to keep per file?</p>
      <div className="slider-wrap onboarding-slider">
        <div className="slider-track">
          <input
            type="range"
            min={1}
            max={10}
            step={1}
            value={retention}
            onChange={(e) => onChangeRetention(Number(e.target.value))}
          />
          <div className="slider-dots">
            {Array.from({ length: 10 }).map((_, idx) => {
              const val = idx + 1;
              const active = val <= retention;
              return (
                <div key={val} className="slider-dot-wrap">
                  <div className={`slider-dot ${active ? 'active' : ''}`} />
                  <div className="slider-num">{val}</div>
                </div>
              );
            })}
          </div>
        </div>
        <div className="muted">
          Keeping <strong>{retention}</strong> old revisions.
        </div>
      </div>
      <div className="muted">You can adjust this anytime in settings.</div>
    </>
  );
}
