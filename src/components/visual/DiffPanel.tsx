import type { FileDiff } from '../../lib/types';

import { type DiffComments, DiffView } from '../DiffView';

const PANEL_LIMIT = 520;

/** One file's real diff, sized to it, in the app's own diff view. */
export function DiffPanel({ comments, diff }: { comments?: DiffComments; diff: FileDiff }) {
    return (
        <div
            className="flex flex-col bg-app"
            style={{ height: panelHeight(diff, comments?.findings.length ?? 0) }}
        >
            <DiffView comments={comments} diffs={[diff]} key={diff.newPath ?? diff.oldPath} />
        </div>
    );
}

/** Tall enough for a short diff and its notes, scrolling past `PANEL_LIMIT`. */
export function panelHeight(diff: FileDiff, notes = 0): number {
    const lines = diff.hunks.reduce((sum, hunk) => sum + hunk.lines.length + 1, 0);

    // The file and hunk headers, and room for a horizontal scrollbar.
    return Math.min(PANEL_LIMIT, 92 + lines * 22 + notes * 110);
}
