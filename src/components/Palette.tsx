import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { useEffect, useMemo, useRef, useState } from 'react';

import { api } from '../lib/api';
import { getSettings, recentRepositories } from '../lib/settings';
import { isPushed, repoName, useRepo } from '../lib/store';
import { Icon } from './Icon';

interface Command {
    group: string;
    hint?: string;
    id: string;
    keys?: string;
    run: () => void;
    title: string;
}

export function Palette() {
    const [query, setQuery] = useState('');
    const [highlighted, setHighlighted] = useState(0);
    const all = useMemo(commands, []);
    const results = useMemo(() => {
        if (!query.trim()) {
            return all;
        }

        return all
            .map(
                (command) =>
                    [command, score(query, `${command.title} ${command.hint ?? ''}`)] as const,
            )
            .filter((entry): entry is readonly [Command, number] => entry[1] !== null)
            .sort((left, right) => left[1] - right[1])
            .map(([c]) => c);
    }, [all, query]);
    const listRef = useRef<HTMLDivElement>(null);
    const close = () => useRepo.getState().set({ palette: false });

    useEffect(() => setHighlighted(0), [query]);
    useEffect(() => {
        listRef.current
            ?.querySelector(`[data-index="${highlighted}"]`)
            ?.scrollIntoView({ block: 'nearest' });
    }, [highlighted]);

    const run = (index: number) => {
        const cmd = results[index];

        if (!cmd) {
            return;
        }

        close();
        cmd.run();
    };

    return (
        <div
            className="fixed inset-0 z-40 flex items-start justify-center bg-black/35 pt-[10vh]"
            onMouseDown={(event) => event.target === event.currentTarget && close()}
        >
            <div
                aria-label="Command palette"
                className="flex w-[600px] max-w-[92vw] flex-col overflow-hidden rounded-xl border border-control-line bg-panel shadow-[0_24px_70px_var(--shadow)]"
                role="dialog"
            >
                <div className="flex items-center gap-2.5 border-b border-line px-4 py-3">
                    <Icon className="text-muted" name="search" size={15} />
                    <input
                        aria-label="Search commands"
                        autoFocus
                        className="flex-1 border-0 bg-transparent text-[15px] outline-none"
                        onChange={(event) => setQuery(event.target.value)}
                        onKeyDown={(event) => {
                            if (event.key === 'ArrowDown') {
                                event.preventDefault();
                                setHighlighted((current) =>
                                    Math.min(current + 1, results.length - 1),
                                );
                            } else if (event.key === 'ArrowUp') {
                                event.preventDefault();
                                setHighlighted((current) => Math.max(current - 1, 0));
                            } else if (event.key === 'Enter') {
                                event.preventDefault();
                                run(highlighted);
                            } else if (event.key === 'Escape') {
                                event.preventDefault();
                                close();
                            }
                        }}
                        placeholder="Search commands"
                        value={query}
                    />
                    <span className="font-mono text-[11px] text-muted">esc</span>
                </div>
                <div className="max-h-[380px] overflow-auto py-1.5" ref={listRef} role="listbox">
                    {results.map((command, index) => (
                        <div key={command.id}>
                            {(index === 0 || results[index - 1].group !== command.group) && (
                                <div className="px-4 pt-2.5 pb-1 text-[11px] font-semibold tracking-[0.08em] text-muted uppercase">
                                    {command.group}
                                </div>
                            )}
                            <button
                                aria-selected={index === highlighted}
                                className={`flex w-full items-center gap-3 px-4 py-[7px] text-left ${index === highlighted ? 'bg-accent-bg' : ''}`}
                                data-index={index}
                                onClick={() => run(index)}
                                onMouseMove={() => setHighlighted(index)}
                                role="option"
                                type="button"
                            >
                                <span>{command.title}</span>
                                {command.hint && (
                                    <span className="truncate text-muted">{command.hint}</span>
                                )}
                                <span className="flex-1" />
                                {command.keys && (
                                    <span className="font-mono text-[12px] text-muted">
                                        {command.keys}
                                    </span>
                                )}
                            </button>
                        </div>
                    ))}
                    {!results.length && (
                        <div className="px-4 py-3 text-muted">No matching commands</div>
                    )}
                </div>
                <div className="flex gap-4 border-t border-line px-4 py-2 text-[12px] text-muted">
                    <span>↑ ↓ move</span>
                    <span>↵ run</span>
                    <span className="flex-1" />
                    <span>Every git action is undoable with ⌘Z</span>
                </div>
            </div>
        </div>
    );
}

function commands(): Command[] {
    const s = useRepo.getState();
    const { inProgress, lastCommit, meta, status } = s;
    const sheet = (next: Parameters<typeof s.set>[0]['sheet']) => () => s.set({ sheet: next });
    const list: Command[] = [];
    const add = (command: Command | false | null | undefined) => command && list.push(command);

    const canCommit = !!s.summary.trim() && status.staged.length > 0 && !status.conflicted.length;

    add(
        canCommit && {
            group: 'Changes',
            hint: `${status.staged.length} staged`,
            id: 'commit',
            keys: '⌘↵',
            run: () => s.commit(),
            title: 'Commit staged changes',
        },
    );
    add(
        (status.unstaged.length || status.untracked.length) > 0 && {
            group: 'Changes',
            id: 'stageAll',
            keys: '⇧⌘A',
            run: () => s.perform({ type: 'stageAll' }, 'Staging'),
            title: 'Stage all',
        },
    );
    add(
        status.staged.length > 0 && {
            group: 'Changes',
            id: 'unstageAll',
            run: () => s.perform({ type: 'unstageAll' }, 'Unstaging'),
            title: 'Unstage all',
        },
    );
    add({
        group: 'Changes',
        id: 'stash',
        keys: '⌥⌘S',
        run: sheet({ kind: 'stash', paths: [] }),
        title: 'Stash changes…',
    });

    add({
        group: 'Remote',
        hint: 'All remotes',
        id: 'fetch',
        keys: '⇧⌘F',
        run: s.fetch,
        title: 'Fetch',
    });
    add({
        group: 'Remote',
        hint: { fastForwardOnly: 'fast-forward only', merge: 'merge', rebase: 'rebase' }[
            getSettings().pullMode
        ],
        id: 'pull',
        keys: '⇧⌘P',
        run: s.pull,
        title: 'Pull',
    });
    add({
        group: 'Remote',
        hint: status.branch.upstream ?? undefined,
        id: 'push',
        keys: '⌘P',
        run: () => s.push(),
        title: status.branch.upstream ? 'Push' : 'Publish branch',
    });
    add({
        group: 'Remote',
        hint: 'Overwrites the remote branch',
        id: 'forcePush',
        run: () => s.push(true),
        title: 'Force push (with lease)',
    });

    add({
        group: 'Branches',
        id: 'newBranch',
        keys: '⇧⌘N',
        run: sheet({ from: null, kind: 'newBranch' }),
        title: 'New branch…',
    });

    for (const b of meta.branches.filter((branch) => !branch.isRemote && !branch.isCurrent)) {
        add({
            group: 'Branches',
            id: `switch-${b.name}`,
            run: () => s.perform({ branch: b.name, type: 'checkout' }, 'Switching branch'),
            title: `Switch to ${b.name}`,
        });
    }

    if (lastCommit) {
        add({
            group: 'History',
            hint: `${lastCommit.sha.slice(0, 7)} · ${isPushed(meta, lastCommit.sha) ? 'pushed' : 'not pushed'}`,
            id: 'rename',
            run: sheet({ commit: lastCommit, kind: 'reword' }),
            title: 'Rename last commit',
        });
        add({
            group: 'History',
            hint: lastCommit.subject,
            id: 'uncommit',
            run: () => s.perform({ type: 'uncommit' }, 'Undoing commit'),
            title: 'Undo last commit (keep changes)',
        });
        add({
            group: 'History',
            hint: 'Adds a commit that undoes it',
            id: 'revert',
            run: () => s.perform({ sha: lastCommit.sha, type: 'revert' }, 'Reverting'),
            title: 'Revert last commit',
        });
    }

    if (inProgress) {
        add({
            group: 'History',
            id: 'abort',
            run: () => s.perform({ type: 'abort' }, 'Aborting'),
            title: 'Abort in-progress operation',
        });
        add(
            !status.conflicted.length && {
                group: 'History',
                id: 'continue',
                run: () => s.perform({ type: 'continue' }, 'Continuing'),
                title: 'Continue in-progress operation',
            },
        );
    }

    add({ group: 'History', id: 'undo', keys: '⌘Z', run: s.undo, title: 'Undo last git action' });
    add({ group: 'History', id: 'redo', keys: '⇧⌘Z', run: s.redo, title: 'Redo' });

    add({
        group: 'View',
        id: 'changes',
        keys: '⌘1',
        run: () => s.set({ tab: 'changes' }),
        title: 'Show changes',
    });
    add({
        group: 'View',
        id: 'history',
        keys: '⌘2',
        run: () => s.set({ tab: 'history' }),
        title: 'Show history',
    });
    add({
        group: 'View',
        id: 'finder',
        run: () => void revealItemInDir(s.root),
        title: 'Show repository in Finder',
    });
    add({
        group: 'View',
        id: 'settings',
        keys: '⌘,',
        run: sheet({ kind: 'settings' }),
        title: 'Settings…',
    });

    add({
        group: 'Repositories',
        id: 'open',
        keys: '⌘O',
        run: () => void api.openFolderDialog(),
        title: 'Open repository…',
    });

    for (const path of recentRepositories().filter((recent) => recent !== s.root)) {
        add({
            group: 'Repositories',
            hint: path.replace(/^\/Users\/[^/]+/, '~'),
            id: `recent-${path}`,
            run: () => void api.openRepository(path),
            title: repoName(path),
        });
    }

    return list;
}

/** Case-insensitive match: substring hits rank first, then subsequence hits. */
function score(query: string, text: string): null | number {
    const q = query.toLowerCase().trim();

    if (!q) {
        return 0;
    }

    const t = text.toLowerCase();
    const i = t.indexOf(q);

    if (i >= 0) {
        return i;
    }

    let pos = 0;
    let total = 100;

    for (const ch of q.replace(/\s/g, '')) {
        const found = t.indexOf(ch, pos);

        if (found < 0) {
            return null;
        }

        total += found - pos;
        pos = found + 1;
    }

    return total;
}
