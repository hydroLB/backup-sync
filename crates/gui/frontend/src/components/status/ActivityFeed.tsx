import React from 'react';
import { UI_TUNING } from '../../config/uiTuning';
import { formatBytes, formatDateTime } from '../../utils/format';

type Activity = {
  path: string;
  bytes: number;
  ts: number;
};

type Props = { items: Activity[] };

const ACTIVITY_FEED_LIMIT = UI_TUNING.activityFeedLimit;

/**
 * Purpose: Render a summary list of recent backup activity.
 *
 * Inputs: `items` as recent activity entries from status polling.
 * Outputs: A React element tree showing the latest activity items.
 * Ties to: Status card and recent activity updates from the backend.
 * Side effects: None.
 * Why: Gives operators quick visibility into recent changes.
 */
const ActivityFeed: React.FC<Props> = ({ items }) => {
  try {
    return (
      <div>
        <div className="section-title">
          <h4 className="heading-compact">Activity</h4>
          <span className="pill">{items?.length || 0} items</span>
        </div>
        <div className="watch-list mt-2">
          {(items || []).slice(0, ACTIVITY_FEED_LIMIT).map((a) => (
            <div key={`${a.path}-${a.ts}`} className="watch-card">
              <div className="watch-card__title">
                <strong>{a.path.split(/[/\\]/).slice(-1)[0]}</strong>
                <span>{a.path}</span>
              </div>
              <div className="muted">
                {formatBytes(a.bytes)} • {formatDateTime(a.ts)}
              </div>
            </div>
          ))}
          {(items || []).length === 0 && <div className="muted">No recent activity yet.</div>}
        </div>
      </div>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[ActivityFeed] Failed to render activity feed: ${reason}`);
  }
};

export default ActivityFeed;
