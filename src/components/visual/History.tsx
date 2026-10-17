import { useEffect, useRef, useState } from 'react';

import type { VisualKind, VisualSummary } from '../../lib/types';

import { useRepo } from '../../lib/store';
import { Icon } from '../Icon';
import { Button, relativeTime } from '../ui';

/** Every saved recap or plan; recaps of the branch you are on come first. */
export function HistoryMenu({
    branch,
    disabled,
    kind,
    shownId,
}: {
    branch: null | string;
    disabled?: boolean;
    kind: VisualKind;
    shownId: null | string;
}) {
    const entries = useRepo((state) => (kind === 'plan' ? state.plans : state.recaps));
    const [open, setOpen] = useState(false);
    const menu = useRef<HTMLDivElement>(null);

    useEffect(() => {
        if (!open) {
            return;
        }

        const close = (event: PointerEvent) =>
            !menu.current?.contains(event.target as Node) && setOpen(false);

        window.addEventListener('pointerdown', close);

        return () => window.removeEventListener('pointerdown', close);
    }, [open]);

    if (!entries.length) {
        return null;
    }

    const here = kind === 'recap' ? entries.filter((entry) => entry.branch === branch) : entries;
    const elsewhere = kind === 'recap' ? entries.filter((entry) => entry.branch !== branch) : [];
    const groups: [string, VisualSummary[]][] =
        kind === 'recap'
            ? [
                  ['This branch', here],
                  ['Other branches', elsewhere],
              ]
            : [['Plans', here]];

    const pick = (entry: VisualSummary) => {
        setOpen(false);
        useRepo.getState().openVisual(kind, entry.id);
    };

    return (
        <div className="relative" ref={menu}>
            <Button
                aria-expanded={open}
                aria-haspopup="menu"
                disabled={disabled}
                onClick={() => setOpen(!open)}
                size="sm"
                title={kind === 'plan' ? 'Earlier plans' : 'Recaps of every branch'}
                variant="ghost"
            >
                <Icon name="undo" size={12} /> History
                <span className="text-muted">{entries.length}</span>
            </Button>
            {open && (
                <div
                    className="absolute top-full right-0 z-20 mt-1 flex max-h-[60vh] w-[360px] flex-col overflow-auto rounded-lg border border-control-line bg-panel p-1 shadow-[0_10px_30px_var(--shadow)]"
                    role="menu"
                >
                    {groups.map(
                        ([label, list]) =>
                            list.length > 0 && (
                                <div className="flex flex-col" key={label}>
                                    <span className="px-2.5 pt-2 pb-1 text-[10.5px] font-semibold tracking-[0.08em] text-muted uppercase">
                                        {label}
                                    </span>
                                    {list.map((entry) => (
                                        <button
                                            className={`flex flex-col gap-0.5 rounded-md px-2.5 py-1.5 text-left hover:bg-hover ${entry.id === shownId ? 'bg-selected' : ''}`}
                                            key={entry.id}
                                            onClick={() => pick(entry)}
                                            role="menuitem"
                                            type="button"
                                        >
                                            <span className="line-clamp-2 leading-snug">
                                                {entry.title}
                                            </span>
                                            <span className="flex items-center gap-1.5 text-[11.5px] text-muted">
                                                {entry.branch && (
                                                    <span className="truncate font-mono">
                                                        {entry.branch}
                                                    </span>
                                                )}
                                                <span className="shrink-0">
                                                    · {relativeTime(entry.createdAt)}
                                                </span>
                                            </span>
                                        </button>
                                    ))}
                                </div>
                            ),
                    )}
                </div>
            )}
        </div>
    );
}
