import type { FileDiff } from '../../lib/types';

import { type DiffComments, DiffView } from '../DiffView';

const PANEL_LIMIT = 520;

/** One file's real diff, sized to it, in the app's own diff view. */
export function DiffPanel({ comments, diff }: { comments?: DiffComments; diff: FileDiff }) {
    return (
        <div className="flex flex-col bg-app">
            <DiffView
                comments={comments}
                diffs={[diff]}
                key={diff.newPath ?? diff.oldPath}
                maxHeight={PANEL_LIMIT}
            />
        </div>
    );
}
