import { openPath, revealItemInDir } from '@tauri-apps/plugin-opener';
import { useState } from 'react';

import type { ChangeKind, FileChange } from '../lib/types';

import { type DiffInteraction, DiffView } from '../components/DiffView';
import { Icon } from '../components/Icon';
import { Button, Checkbox, Collapsible, relativeTime, SectionLabel } from '../components/ui';
import { updateSettings, useSettings } from '../lib/settings';
import { isPushed, useRepo } from '../lib/store';

export function ChangesView() {
    const { leftPanel, rightPanel } = useSettings();
    const staged = useRepo((state) => state.status.staged.length);

    return (
        <>
            <Collapsible open={leftPanel} width={264}>
                <FileList />
            </Collapsible>
            <DiffPane />
            <Collapsible open={rightPanel} width={330}>
                <CommitPanel />
            </Collapsible>
            {/* While the commit panel is hidden, a rail on the right edge brings it back. */}
            <Collapsible open={!rightPanel} width={36}>
                <button
                    aria-label="Show commit panel"
                    className="flex h-full w-full flex-col items-center gap-3 border-l border-line bg-panel py-3 text-muted hover:bg-hover hover:text-fg"
                    onClick={() => updateSettings({ rightPanel: true })}
                    title="Show commit panel (⌘L)"
                    type="button"
                >
                    <Icon name="sidebarRight" size={15} />
                    <span className="text-[12px] tracking-wide [writing-mode:vertical-rl]">
                        Commit
                    </span>
                    {staged > 0 && (
                        <span className="rounded-full bg-accent-bg px-1.5 font-mono text-[11px] text-accent">
                            {staged}
                        </span>
                    )}
                </button>
            </Collapsible>
        </>
    );
}

const kindLetter: Record<ChangeKind, string> = {
    added: 'A',
    conflicted: '!',
    copied: 'C',
    deleted: 'D',
    modified: 'M',
    renamed: 'R',
    typeChanged: 'T',
    untracked: 'U',
};

const kindColor: Record<ChangeKind, string> = {
    added: 'text-add',
    conflicted: 'text-warn',
    copied: 'text-add',
    deleted: 'text-del',
    modified: 'text-muted',
    renamed: 'text-muted',
    typeChanged: 'text-muted',
    untracked: 'text-add',
};

const kindName: Record<ChangeKind, string> = {
    added: 'Added',
    conflicted: 'Conflicted',
    copied: 'Copied',
    deleted: 'Deleted',
    modified: 'Modified',
    renamed: 'Renamed',
    typeChanged: 'Type changed',
    untracked: 'New file',
};

function CommitPanel() {
    const s = useRepo();
    const { amend, body, busy, lastCommit, meta, status, summary } = s;
    const staged = status.staged.length;
    const canCommit =
        !!summary.trim() && (amend || staged > 0) && !status.conflicted.length && !busy;
    const lastPushed = lastCommit
        ? isPushed(meta, lastCommit.sha) && !!status.branch.upstream
        : false;

    return (
        <aside
            aria-label="Commit"
            className="flex h-full w-[330px] flex-col gap-3.5 overflow-auto border-l border-line bg-panel p-4"
        >
            <div className="flex items-center justify-between gap-2">
                <h2 className="m-0 text-[14px] font-semibold">
                    {amend
                        ? `Amend ${lastCommit?.sha.slice(0, 7) ?? ''}`
                        : staged
                          ? `Commit ${staged} staged file${staged === 1 ? '' : 's'}`
                          : 'Commit'}
                </h2>
                <Button
                    aria-label="Hide commit panel"
                    onClick={() => updateSettings({ rightPanel: false })}
                    size="sm"
                    title="Hide commit panel (⌘L)"
                    variant="ghost"
                >
                    <Icon name="sidebarRight" size={14} />
                </Button>
            </div>
            <div className="flex flex-col gap-1.5">
                <div className="flex justify-between text-[12px] text-muted">
                    <label htmlFor="summary">Summary</label>
                    <span className={`font-mono ${summary.length > 72 ? 'text-del' : ''}`}>
                        {summary.length} / 72
                    </span>
                </div>
                <input
                    className="h-9 rounded-[7px] border border-control-line bg-app px-2.5 outline-none focus:border-accent"
                    id="summary"
                    onChange={(event) => s.set({ summary: event.target.value })}
                    placeholder="What does this change do?"
                    value={summary}
                />
            </div>
            <div className="flex flex-col gap-1.5">
                <label className="text-[12px] text-muted" htmlFor="description">
                    Description
                </label>
                <textarea
                    className="resize-y rounded-[7px] border border-control-line bg-app px-2.5 py-2 leading-normal outline-none focus:border-accent"
                    id="description"
                    onChange={(event) => s.set({ body: event.target.value })}
                    rows={6}
                    value={body}
                />
            </div>
            <Checkbox
                checked={amend}
                label={<span className={lastCommit ? '' : 'text-muted'}>Amend last commit</span>}
                onChange={s.setAmend}
            />
            <div className="flex flex-col gap-2">
                <Button
                    disabled={!canCommit}
                    onClick={() => s.commit()}
                    size="lg"
                    variant="primary"
                >
                    {amend ? 'Amend commit' : 'Commit'}{' '}
                    <span className="font-mono text-[11px] opacity-70">⌘↵</span>
                </Button>
                <Button disabled={!canCommit || amend} onClick={() => s.commit(true)}>
                    Commit &amp; push
                </Button>
                {status.conflicted.length > 0 ? (
                    <span className="text-[12px] text-warn">
                        Resolve conflicts before committing.
                    </span>
                ) : !staged && !amend ? (
                    <span className="text-[12px] text-muted">
                        Stage files, hunks or lines to commit them. Click a line number to pick
                        single lines.
                    </span>
                ) : null}
            </div>
            <div className="flex-1" />
            {lastCommit && (
                <div className="flex flex-col gap-2 border-t border-line pt-3.5">
                    <SectionLabel>Last commit</SectionLabel>
                    <div className="flex flex-col gap-0.5">
                        <span className="line-clamp-2">{lastCommit.subject}</span>
                        <span className="font-mono text-[11.5px] text-muted">
                            {lastCommit.sha.slice(0, 7)} · {relativeTime(lastCommit.date)}
                        </span>
                    </div>
                    <Button
                        disabled={lastPushed}
                        onClick={() => s.perform({ type: 'uncommit' }, 'Undoing commit')}
                        title={
                            lastPushed
                                ? 'Already pushed: use Revert in History instead.'
                                : 'Moves the commit’s changes back to staged.'
                        }
                    >
                        <Icon name="undo" size={13} /> Undo commit (keep changes)
                    </Button>
                </div>
            )}
        </aside>
    );
}

function ConflictPanel({ change }: { change: FileChange }) {
    const { perform, root } = useRepo();
    const resolve = (side: 'ours' | 'theirs' | null) =>
        perform({ path: change.path, side, type: 'resolve' }, 'Resolving');

    return (
        <div className="flex flex-1 flex-col items-center justify-center gap-4 p-6 text-center">
            <Icon className="text-warn" name="merge" size={34} />
            <h3 className="m-0 text-[16px] font-semibold">{change.path} has conflicts</h3>
            <p className="m-0 max-w-md text-muted">
                Edit the file to resolve the conflict markers, then mark it resolved — or take one
                side as a whole.
            </p>
            <div className="flex flex-wrap justify-center gap-2">
                <Button onClick={() => void openPath(`${root}/${change.path}`)}>
                    Open in editor
                </Button>
                <Button onClick={() => resolve('ours')} variant="add">
                    Take ours
                </Button>
                <Button onClick={() => resolve('theirs')} variant="del">
                    Take theirs
                </Button>
                <Button onClick={() => resolve(null)} variant="primary">
                    Mark as resolved
                </Button>
            </div>
        </div>
    );
}

function DiffPane() {
    const { applyLines, diff, lineSelection, loadDiff, perform, root, selected, set, toggleLine } =
        useRepo();
    const { ignoreWhitespace, splitDiff } = useSettings();

    if (!selected) {
        return (
            <div className="flex flex-1 items-center justify-center text-muted">
                Select a file to see its changes.
            </div>
        );
    }

    const slash = selected.path.lastIndexOf('/');
    const interaction: DiffInteraction = {
        actions:
            selected.area === 'staged'
                ? [{ action: 'unstage', title: 'Unstage', variant: 'default' }]
                : [
                      { action: 'stage', title: 'Stage', variant: 'add' },
                      { action: 'discard', title: 'Discard', variant: 'del' },
                  ],
        apply: applyLines,
        clear: (hunk) => {
            const next = { ...lineSelection };

            delete next[hunk];
            set({ lineSelection: next });
        },
        selection: lineSelection,
        toggle: toggleLine,
    };

    return (
        <section aria-label="Diff" className="flex min-w-0 flex-1 flex-col">
            <div className="flex shrink-0 flex-wrap items-center gap-2.5 border-b border-line px-[18px] py-2.5">
                <span className="font-mono text-[12.5px]">
                    <span className="text-muted">{selected.path.slice(0, slash + 1)}</span>
                    {selected.path.slice(slash + 1)}
                </span>
                {diff && !diff.isBinary && (
                    <span className="font-mono text-[12px]">
                        <span className="text-add">+{diff.additions}</span>{' '}
                        <span className="text-del">−{diff.deletions}</span>
                    </span>
                )}
                <span className="rounded-full bg-control px-2 py-0.5 text-[11.5px] text-muted">
                    {
                        {
                            conflicted: 'Conflict',
                            staged: 'Staged',
                            unstaged: 'Not staged',
                            untracked: 'New file',
                        }[selected.area]
                    }
                </span>
                <span className="flex-1" />
                <Checkbox
                    checked={ignoreWhitespace}
                    label={<span className="text-[12px] text-soft">Ignore whitespace</span>}
                    onChange={(checked) => {
                        updateSettings({ ignoreWhitespace: checked });
                        void loadDiff();
                    }}
                />
                <div
                    aria-label="Diff layout"
                    className="flex gap-0.5 rounded-md border border-control-line bg-control p-0.5 text-[12px]"
                    role="radiogroup"
                >
                    {[false, true].map((split) => (
                        <button
                            aria-checked={splitDiff === split}
                            className={`rounded px-2 py-0.5 ${splitDiff === split ? 'bg-tab text-fg' : 'text-muted'}`}
                            key={String(split)}
                            onClick={() => updateSettings({ splitDiff: split })}
                            role="radio"
                            type="button"
                        >
                            {split ? 'Split' : 'Unified'}
                        </button>
                    ))}
                </div>
                {selected.area === 'staged' && (
                    <Button
                        onClick={() =>
                            perform({ paths: [selected.path], type: 'unstage' }, 'Unstaging')
                        }
                        size="sm"
                    >
                        Unstage file
                    </Button>
                )}
                {(selected.area === 'unstaged' || selected.area === 'untracked') && (
                    <Button
                        onClick={() =>
                            perform({ paths: [selected.path], type: 'stage' }, 'Staging')
                        }
                        size="sm"
                        variant="add"
                    >
                        Stage file
                    </Button>
                )}
                <Button
                    aria-label="Open in editor"
                    onClick={() => void openPath(`${root}/${selected.path}`)}
                    size="sm"
                    title="Open in default editor"
                    variant="ghost"
                >
                    <Icon name="external" size={13} />
                </Button>
            </div>
            {selected.area === 'conflicted' ? (
                <ConflictPanel change={selected} />
            ) : diff ? (
                <DiffView diffs={[diff]} interaction={interaction} />
            ) : (
                <div className="flex flex-1 items-center justify-center text-muted">
                    {ignoreWhitespace ? 'Only whitespace changed.' : 'No textual changes.'}
                </div>
            )}
        </section>
    );
}

function FileList() {
    const { perform, select, selected, set, status } = useRepo();
    const [menu, setMenu] = useState<{ change: FileChange; x: number; y: number } | null>(null);
    const unstaged = [...status.unstaged, ...status.untracked];
    const isClean = !status.staged.length && !unstaged.length && !status.conflicted.length;

    const row = (change: FileChange) => {
        const active = selected?.path === change.path && selected.area === change.area;
        const slash = change.path.lastIndexOf('/');
        const toggle = () =>
            change.area === 'staged'
                ? perform({ paths: [change.path], type: 'unstage' }, 'Unstaging')
                : change.area !== 'conflicted' &&
                  perform({ paths: [change.path], type: 'stage' }, 'Staging');

        return (
            <li className="group relative" key={`${change.area}:${change.path}`}>
                <button
                    aria-current={active || undefined}
                    className={`flex w-full items-center gap-2 rounded-md py-[7px] pr-8 pl-2 text-left ${active ? 'bg-selected' : 'hover:bg-hover'}`}
                    onClick={() => select(change)}
                    onContextMenu={(event) => {
                        event.preventDefault();
                        select(change);
                        setMenu({ change, x: event.clientX, y: event.clientY });
                    }}
                    onDoubleClick={toggle}
                    type="button"
                >
                    <span
                        className={`w-3 shrink-0 font-mono text-[11px] font-semibold ${kindColor[change.kind]}`}
                        title={kindName[change.kind]}
                    >
                        {kindLetter[change.kind]}
                    </span>
                    <span className="min-w-0 flex-1">
                        <span className="block truncate">{change.path.slice(slash + 1)}</span>
                        {(slash > 0 || change.originalPath) && (
                            <span className="block truncate text-[11.5px] text-muted" dir="rtl">
                                {change.originalPath
                                    ? `from ${change.originalPath}`
                                    : change.path.slice(0, slash + 1)}
                            </span>
                        )}
                    </span>
                </button>
                {change.area !== 'conflicted' && (
                    <button
                        aria-label={
                            change.area === 'staged'
                                ? `Unstage ${change.path}`
                                : `Stage ${change.path}`
                        }
                        className="absolute top-1/2 right-1.5 hidden h-6 w-6 -translate-y-1/2 items-center justify-center rounded-md border border-control-line bg-control text-muted group-hover:flex hover:text-fg focus:flex"
                        onClick={toggle}
                        title={change.area === 'staged' ? 'Unstage' : 'Stage'}
                        type="button"
                    >
                        <Icon name={change.area === 'staged' ? 'minus' : 'plus'} size={12} />
                    </button>
                )}
            </li>
        );
    };

    const section = (
        title: string,
        items: FileChange[],
        action?: { label: string; run: () => void },
        warn = false,
    ) =>
        items.length > 0 && (
            <section className="flex flex-col gap-0.5">
                <div className="flex items-center justify-between px-2 pb-1.5">
                    <SectionLabel className={warn ? 'text-warn' : ''}>
                        {title} · {items.length}
                    </SectionLabel>
                    {action && (
                        <button
                            className="text-[12px] text-muted hover:text-fg"
                            onClick={action.run}
                            type="button"
                        >
                            {action.label}
                        </button>
                    )}
                </div>
                <ul className="m-0 list-none p-0">{items.map(row)}</ul>
            </section>
        );

    return (
        <nav
            aria-label="Changed files"
            className="flex h-full w-[264px] flex-col border-r border-line bg-panel"
        >
            <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-auto px-2 py-3.5">
                {section('Conflicts', status.conflicted, undefined, true)}
                {section('Staged', status.staged, {
                    label: 'Unstage all',
                    run: () => perform({ type: 'unstageAll' }, 'Unstaging'),
                })}
                {section('Unstaged', unstaged, {
                    label: 'Stage all',
                    run: () => perform({ type: 'stageAll' }, 'Staging'),
                })}
                {isClean && (
                    <div className="flex flex-1 flex-col items-center justify-center gap-2 px-4 text-center text-muted">
                        <Icon className="text-accent" name="check" size={22} />
                        <span className="font-semibold text-fg">No changes</span>
                        <span className="text-[12px]">
                            Edit files in this folder and they show up here.
                        </span>
                    </div>
                )}
            </div>
            {!isClean && (
                <div className="flex gap-2 border-t border-line p-2.5">
                    <Button onClick={() => set({ sheet: { kind: 'stash', paths: [] } })} size="sm">
                        Stash…
                    </Button>
                    <span className="flex-1" />
                    <Button
                        disabled={!unstaged.length}
                        onClick={() => set({ sheet: { changes: unstaged, kind: 'discard' } })}
                        size="sm"
                        variant="danger"
                    >
                        Discard…
                    </Button>
                </div>
            )}
            {menu && <FileMenu {...menu} onClose={() => setMenu(null)} />}
        </nav>
    );
}

function FileMenu({
    change,
    onClose,
    x,
    y,
}: {
    change: FileChange;
    onClose: () => void;
    x: number;
    y: number;
}) {
    const { perform, root, set } = useRepo();
    const items: [string, () => void, boolean?][] = [];

    if (change.area === 'staged') {
        items.push([
            'Unstage',
            () => perform({ paths: [change.path], type: 'unstage' }, 'Unstaging'),
        ]);
    }

    if (change.area === 'unstaged' || change.area === 'untracked') {
        items.push(['Stage', () => perform({ paths: [change.path], type: 'stage' }, 'Staging')]);
        items.push([
            'Discard changes…',
            () => set({ sheet: { changes: [change], kind: 'discard' } }),
            true,
        ]);
        items.push([
            'Stash this file…',
            () => set({ sheet: { kind: 'stash', paths: [change.path] } }),
        ]);
    }

    if (change.area === 'conflicted') {
        items.push([
            'Mark as resolved',
            () => perform({ path: change.path, side: null, type: 'resolve' }, 'Resolving'),
        ]);
        items.push([
            'Take ours',
            () => perform({ path: change.path, side: 'ours', type: 'resolve' }, 'Resolving'),
        ]);
        items.push([
            'Take theirs',
            () => perform({ path: change.path, side: 'theirs', type: 'resolve' }, 'Resolving'),
        ]);
    }

    items.push(['Open', () => void openPath(`${root}/${change.path}`)]);
    items.push(['Show in Finder', () => void revealItemInDir(`${root}/${change.path}`)]);
    items.push(['Copy path', () => void navigator.clipboard.writeText(change.path)]);

    return (
        <>
            <div
                className="fixed inset-0 z-30"
                onClick={onClose}
                onContextMenu={(event) => (event.preventDefault(), onClose())}
            />
            <div
                className="fixed z-40 min-w-48 rounded-lg border border-control-line bg-panel p-1 shadow-[0_12px_40px_var(--shadow)]"
                role="menu"
                style={{ left: x, top: y }}
            >
                {items.map(([label, run, danger]) => (
                    <button
                        className={`block w-full rounded-md px-2.5 py-1.5 text-left hover:bg-hover ${danger ? 'text-warn' : ''}`}
                        key={label}
                        onClick={() => {
                            onClose();
                            run();
                        }}
                        role="menuitem"
                        type="button"
                    >
                        {label}
                    </button>
                ))}
            </div>
        </>
    );
}
