import React, { useState } from 'react';
import { validateIgnorePatterns } from '../utils/validation';
import { UI_TUNING } from '../config/uiTuning';

type Props = {
  skip_hidden: boolean;
  ignore_patterns: string[];
  max_parallel_copies: number;
  max_bytes_per_second: number | null;
  min_free_space_bytes?: number | null;
  onChange: (
    data: Partial<{
      skip_hidden: boolean;
      ignore_patterns: string[];
      max_parallel_copies: number;
      max_bytes_per_second: number | null;
      min_free_space_bytes?: number | null;
    }>,
  ) => void;
  onStatus: (msg: string) => void;
};

/**
 * Purpose: Render performance tuning controls and presets.
 *
 * Inputs: Current tuning values, change handlers, and status callback.
 * Outputs: A tuning control panel element.
 * Ties to: Settings panel performance section.
 * Side effects: Registers React state hooks for advanced toggles.
 * Why: Make execution tuning simple and discoverable.
 */
const PerformanceControls: React.FC<Props> = ({
  skip_hidden,
  ignore_patterns,
  max_parallel_copies,
  max_bytes_per_second,
  min_free_space_bytes,
  onChange,
  onStatus,
}) => {
  const [showAdvanced, setShowAdvanced] = useState<boolean>(false);

  /**
   * Purpose: Apply a performance preset to the current settings.
   *
   * Inputs: `preset` as the desired tuning profile.
   * Outputs: Updates configuration and status messaging.
   * Ties to: Performance presets in `UI_TUNING`.
   * Side effects: Updates configuration state and status messaging.
   * Why: Provide quick, consistent tuning options.
   */
  const applyPreset = (preset: 'quiet' | 'balanced' | 'fast') => {
    try {
      if (preset === 'quiet') {
        onChange(UI_TUNING.performancePresets.quiet);
        onStatus('Preset: Quiet (1 copy, 5 MB/s)');
      }
      if (preset === 'balanced') {
        onChange(UI_TUNING.performancePresets.balanced);
        onStatus('Preset: Balanced (2 copies, no cap)');
      }
      if (preset === 'fast') {
        onChange(UI_TUNING.performancePresets.fast);
        onStatus('Preset: Fast (4 copies, no cap)');
      }
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      onStatus(`[PerformanceControls::applyPreset] Failed to apply preset: ${reason}`);
    }
  };

  /**
   * Purpose: Add a standard set of ignore patterns to the config.
   *
   * Inputs: None.
   * Outputs: Updates ignore patterns and status messaging.
   * Ties to: Common ignore patterns in `UI_TUNING`.
   * Side effects: Updates configuration state and status messaging.
   * Why: Reduce noisy backups with one click defaults.
   */
  const applyCommonIgnores = () => {
    try {
      const merged = Array.from(
        new Set([...(ignore_patterns || []), ...UI_TUNING.commonIgnorePatterns]),
      );
      onChange({ ignore_patterns: merged });
      onStatus('Added common ignores');
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      onStatus(
        `[PerformanceControls::applyCommonIgnores] Failed to add ignore patterns: ${reason}`,
      );
    }
  };

  /**
   * Purpose: Toggle the advanced tuning section visibility.
   *
   * Inputs: None.
   * Outputs: Updates local state.
   * Ties to: Advanced control visibility in the settings panel.
   * Side effects: Updates React state for visibility.
   * Why: Keep the panel compact unless advanced controls are needed.
   */
  const toggleAdvanced = () => {
    try {
      setShowAdvanced(!showAdvanced);
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      onStatus(
        `[PerformanceControls::toggleAdvanced] Failed to toggle advanced controls: ${reason}`,
      );
    }
  };

  return (
    <>
      <div className="inline-actions">
        <button className="btn secondary" onClick={() => applyPreset('quiet')}>
          Quiet
        </button>
        <button className="btn secondary" onClick={() => applyPreset('balanced')}>
          Balanced
        </button>
        <button className="btn secondary" onClick={() => applyPreset('fast')}>
          Fast
        </button>
        <button className="btn secondary" onClick={applyCommonIgnores}>
          Add common ignores
        </button>
        <button className="btn secondary" onClick={toggleAdvanced}>
          {showAdvanced ? 'Hide advanced' : 'Advanced'}
        </button>
      </div>
      {showAdvanced && (
        <>
          <label style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
            <input
              type="checkbox"
              checked={skip_hidden}
              onChange={(e) => onChange({ skip_hidden: e.target.checked })}
            />
            <span title="Helps avoid backing up OS clutter and dotfiles.">
              Skip hidden files/folders (recommended)
            </span>
          </label>
          <label>
            Ignore patterns (one glob per line)
            <textarea
              value={(ignore_patterns || []).join('\n')}
              onChange={(e) => {
                const next = e.target.value.split('\n').filter(Boolean);
                const err = validateIgnorePatterns(next);
                if (err) {
                  onStatus(err);
                } else {
                  onChange({ ignore_patterns: next });
                }
              }}
              style={{
                minHeight: 80,
                background: 'rgba(255,255,255,0.04)',
                color: 'var(--text)',
                border: '1px solid var(--border)',
                borderRadius: 10,
                padding: 10,
              }}
            />
            <div
              className="muted"
              style={{ marginTop: 4 }}
              title="Use ** for folders, *.ext for files."
            >
              Invalid globs will be blocked on save; use ** for folders, *.ext for files.
            </div>
          </label>
          <label>
            Max parallel copies
            <input
              type="number"
              value={max_parallel_copies}
              onChange={(e) => onChange({ max_parallel_copies: Number(e.target.value) || 1 })}
            />
          </label>
          <label>
            Max bytes per second (0 = unlimited)
            <input
              type="number"
              value={max_bytes_per_second ?? 0}
              onChange={(e) => {
                const val = Number(e.target.value);
                onChange({ max_bytes_per_second: val > 0 ? val : null });
              }}
            />
          </label>
          <label>
            Minimum free space to continue (bytes, 0 = off)
            <input
              type="number"
              value={min_free_space_bytes ?? 0}
              onChange={(e) => {
                const val = Number(e.target.value);
                onChange({ min_free_space_bytes: val > 0 ? val : null });
              }}
            />
          </label>
        </>
      )}
    </>
  );
};

export default PerformanceControls;
