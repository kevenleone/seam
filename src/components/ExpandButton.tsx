import { useRepo } from '../lib/store';
import { Icon } from './Icon';
import { Button } from './ui';

/** Slides everything but the diff away, and back (⇧⌘E, or esc). */
export function ExpandButton() {
    const expanded = useRepo((state) => state.expanded);
    const set = useRepo((state) => state.set);
    const label = expanded ? 'Show panels (esc)' : 'Expand the diff (⇧⌘E)';

    return (
        <Button
            aria-label={label}
            aria-pressed={expanded}
            className={expanded ? 'text-accent' : ''}
            onClick={() => set({ expanded: !expanded })}
            size="sm"
            title={label}
            variant="ghost"
        >
            <Icon name={expanded ? 'collapse' : 'expand'} size={13} />
        </Button>
    );
}
