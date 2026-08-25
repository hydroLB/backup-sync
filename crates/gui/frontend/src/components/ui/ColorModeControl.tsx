import { useEffect, useRef, useState } from 'react';
import { ColorModePreference } from '../../theme/tokens';

type Props = {
  preference: ColorModePreference;
  onChange: (mode: ColorModePreference) => void;
};

const MODE_OPTIONS: Array<{ mode: ColorModePreference; label: string; ariaLabel: string }> = [
  { mode: 'light', label: 'Light', ariaLabel: 'Use light mode' },
  { mode: 'dark', label: 'Dark', ariaLabel: 'Use dark mode' },
  { mode: 'auto', label: 'Auto', ariaLabel: 'Match system appearance' },
];

/** Provide explicit mode selection while keeping auto mode discoverable. */
export function ColorModeControl({ preference, onChange }: Props) {
  try {
    const [expanded, setExpanded] = useState(false);
    const rootRef = useRef<HTMLDivElement | null>(null);
    const activeOption =
      MODE_OPTIONS.find((option) => option.mode === preference) ?? MODE_OPTIONS[2]!;

    useEffect(() => {
      if (!expanded) {
        return;
      }

      const handlePointerDown = (event: PointerEvent) => {
        if (rootRef.current?.contains(event.target as Node)) {
          return;
        }
        setExpanded(false);
      };

      const handleEscape = (event: KeyboardEvent) => {
        if (event.key === 'Escape') {
          setExpanded(false);
        }
      };

      window.addEventListener('pointerdown', handlePointerDown);
      window.addEventListener('keydown', handleEscape);
      return () => {
        window.removeEventListener('pointerdown', handlePointerDown);
        window.removeEventListener('keydown', handleEscape);
      };
    }, [expanded]);

    return (
      <div
        ref={rootRef}
        className={`segmented-control segmented-control--compact${expanded ? ' is-expanded' : ''}`}
      >
        <button
          type="button"
          className="segmented-trigger"
          aria-haspopup="menu"
          aria-expanded={expanded}
          aria-label={`Theme control, current mode ${activeOption.label}`}
          onClick={() => setExpanded((previous) => !previous)}
        >
          <span className="segmented-trigger-label">Theme</span>
          <span className="segmented-trigger-value">{activeOption.label}</span>
          <span className="segmented-trigger-caret" aria-hidden="true">
            {expanded ? '−' : '+'}
          </span>
        </button>
        {expanded && (
          <div className="segmented-menu" role="menu" aria-label="Color mode">
            {MODE_OPTIONS.map((option) => {
              const selected = option.mode === preference;
              return (
                <button
                  key={option.mode}
                  type="button"
                  className="segmented-tab segmented-menu-option"
                  role="menuitemradio"
                  aria-label={option.ariaLabel}
                  aria-checked={selected}
                  aria-pressed={selected}
                  onClick={() => {
                    onChange(option.mode);
                    setExpanded(false);
                  }}
                >
                  <span>{option.label}</span>
                  <span className="segmented-option-state">{selected ? 'Active' : 'Select'}</span>
                </button>
              );
            })}
          </div>
        )}
      </div>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[ColorModeControl] Failed to render color mode control: ${reason}`);
  }
}
