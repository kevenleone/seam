import { useState } from 'react';

import type { Branch, Commit } from '../lib/types';

import { DiffView } from '../components/DiffView';
import { Icon } from '../components/Icon';
import { Button, Collapsible, relativeTime, SectionLabel } from '../components/ui';
import { useSettings } from '../lib/settings';
import { branchLabel, isPushed, useRepo } from '../lib/store';

export function ContextMenu({
    children,
    onClose,
    x,
    y,
}: {
    children: readonly (readonly [string, () => void])[];
    onClose: () => void;
    x: number;
    y: number;
}) {
    return (
        <>
            <div
                className="fixed inset-0 z-30"
                onClick={onClose}
                onContextMenu={(event) => (event.preventDefault(), onClose())}
            />
            <div
                className="fixed z-40 min-w-52 rounded-lg border border-control-line bg-panel p-1 shadow-[0_12px_40px_var(--shadow)]"
                role="menu"
                style={{ left: x, top: y }}
            >
                {children.map(([label, run]) => (
                    <button
                        className={`block w-full rounded-md px-2.5 py-1.5 text-left hover:bg-hover ${label.startsWith('Delete') || label.startsWith('Remove') ? 'text-warn' : ''}`}
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

export function HistoryView() {
    const { leftPanel } = useSettings();

    return (
        <>
            <Collapsible open={leftPanel} width={250}>
                <Sidebar />
            </Collapsible>
            <CommitList />
            <Detail />
        </>
    );
}

function cherryPick(shas: string[]) {
    const s = useRepo.getState();
    // Oldest first, so they apply in order.
    const ordered = s.commits
        .filter((commit) => shas.includes(commit.sha))
        .reverse()
        .map((commit) => commit.sha);

    s.perform({ shas: ordered, type: 'cherryPick' }, 'Cherry-picking');
    s.setChecked([]);
}

/** Header for the diff of ticked commits: everything from the oldest one to the newest. */
function CombinedHeader({ shas }: { shas: string[] }) {
    const commits = useRepo((state) => state.commits);
    // History lists newest first.
    const ordered = commits.filter((commit) => shas.includes(commit.sha));
    const newest = ordered[0];
    const oldest = ordered[ordered.length - 1];

    if (!newest || !oldest) {
        return null;
    }

    const first = commits.indexOf(newest);
    const last = commits.indexOf(oldest);
    const between = last - first + 1 - ordered.length;

    return (
        <>
            <div className="flex items-center gap-2 font-mono text-[12.5px]">
                <span>{oldest.sha.slice(0, 7)}</span>
                <span className="text-muted">…</span>
                <span>{newest.sha.slice(0, 7)}</span>
                <span className="rounded-full bg-accent-bg px-2 py-0.5 font-sans text-[11.5px] text-accent">
                    {ordered.length} selected
                </span>
            </div>
            <h2 className="m-0 text-[16px] leading-snug font-semibold">
                Changes from “{oldest.subject}” to “{newest.subject}”
            </h2>
            <span className="text-[12px] text-muted">
                Everything from the parent of the oldest selected commit to the newest
                {between > 0
                    ? `, including ${between} unselected commit${between === 1 ? '' : 's'} in between.`
                    : '.'}
            </span>
        </>
    );
}

function commitActions(commit: Commit, current: boolean): (readonly [string, () => void])[] {
    const s = useRepo.getState();
    const items: (readonly [string, () => void])[] = current
        ? [
              ['Rename…', () => s.set({ sheet: { commit: commit, kind: 'reword' } })],
              ['Remove…', () => s.set({ sheet: { kind: 'drop', shas: [commit.sha] } })],
              ['Revert', () => s.perform({ sha: commit.sha, type: 'revert' }, 'Reverting')],
              ['Reset branch to here…', () => s.set({ sheet: { commit: commit, kind: 'reset' } })],
          ]
        : [[`Cherry-pick onto ${branchLabel(s.status)}`, () => cherryPick([commit.sha])]];

    return [
        ...items,
        ['New branch from here…', () => s.set({ sheet: { from: commit.sha, kind: 'newBranch' } })],
        ['Tag…', () => s.set({ sheet: { commit: commit, kind: 'tag' } })],
        ['Copy SHA', () => void navigator.clipboard.writeText(commit.sha)],
    ];
}

function CommitList() {
    const s = useRepo();
    const { baseBranch, baseCommit, checked, commits, detail, hasMore, historyRef, meta, status } =
        s;
    const current = viewingCurrent(historyRef, status.branch.head);
    const [menu, setMenu] = useState<{ commit: Commit; x: number; y: number } | null>(null);
    const refName =
        historyRef === 'HEAD'
            ? branchLabel(status)
            : historyRef.replace(/^refs\/(heads|remotes)\//, '');

    const toggle = (sha: string) =>
        s.setChecked(
            checked.includes(sha)
                ? checked.filter((checkedSha) => checkedSha !== sha)
                : [...checked, sha],
        );

    return (
        <section
            aria-label="Commits"
            className="flex w-[440px] shrink-0 flex-col border-r border-line"
        >
            {checked.length > 0 ? (
                <div className="flex flex-wrap items-center gap-2 border-b border-accent-line bg-accent-bg px-3.5 py-2.5">
                    <span className="font-semibold">{checked.length} selected</span>
                    <span className="flex-1" />
                    {current ? (
                        <>
                            <Button
                                disabled={checked.length < 2}
                                onClick={() => s.set({ sheet: { kind: 'squash', shas: checked } })}
                                size="sm"
                            >
                                Squash…
                            </Button>
                            <Button
                                onClick={() => s.set({ sheet: { kind: 'drop', shas: checked } })}
                                size="sm"
                                variant="danger"
                            >
                                Remove…
                            </Button>
                        </>
                    ) : (
                        <Button onClick={() => cherryPick(checked)} size="sm">
                            Cherry-pick onto {branchLabel(status)}
                        </Button>
                    )}
                    <Button onClick={() => s.setChecked([])} size="sm" variant="ghost">
                        Clear
                    </Button>
                </div>
            ) : (
                <div className="flex items-center gap-2 border-b border-line px-3.5 py-2.5">
                    <span className="font-mono text-[12.5px] font-semibold">{refName}</span>
                    {!current && (
                        <button
                            className="text-[12px] text-accent hover:text-accent-strong"
                            onClick={() => s.viewHistory('HEAD')}
                            type="button"
                        >
                            Back to {branchLabel(status)}
                        </button>
                    )}
                    <span className="flex-1" />
                    <span className="truncate text-[12px] text-muted">
                        Tick commits to squash, remove or cherry-pick them.
                    </span>
                </div>
            )}
            <ol className="m-0 min-h-0 flex-1 list-none overflow-auto p-0">
                {commits.map((commit, index) => {
                    const active = detail?.kind === 'commit' && detail.sha === commit.sha;
                    const tags = commit.refs
                        .filter((ref) => ref.startsWith('tag: '))
                        .map((ref) => ref.slice(5));

                    return (
                        <li key={commit.sha}>
                            {commit.sha === baseCommit && index > 0 && baseBranch && (
                                <div className="flex items-center gap-2 px-3.5 py-2 text-[11.5px] text-muted">
                                    <span className="h-px flex-1 bg-line" />
                                    {baseBranch} branched here · everything above is only on{' '}
                                    {refName}
                                    <span className="h-px flex-1 bg-line" />
                                </div>
                            )}
                            <div
                                className={`flex cursor-default items-center gap-3 border-b border-line/60 px-3.5 py-2 ${active ? 'bg-selected' : 'hover:bg-hover'}`}
                                onClick={() => s.showDetail({ kind: 'commit', sha: commit.sha })}
                                onContextMenu={(event) => {
                                    event.preventDefault();
                                    s.showDetail({ kind: 'commit', sha: commit.sha });
                                    setMenu({ commit: commit, x: event.clientX, y: event.clientY });
                                }}
                            >
                                <input
                                    aria-label={`Select ${commit.subject}`}
                                    checked={checked.includes(commit.sha)}
                                    className="m-0 accent-[var(--accent)]"
                                    onChange={() => toggle(commit.sha)}
                                    onClick={(event) => event.stopPropagation()}
                                    type="checkbox"
                                />
                                <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                                    <span className="flex items-center gap-1.5">
                                        <span className="truncate">{commit.subject}</span>
                                        {tags.map((tag) => (
                                            <span
                                                className="shrink-0 rounded-full bg-accent-bg px-1.5 font-mono text-[10.5px] text-accent"
                                                key={tag}
                                            >
                                                {tag}
                                            </span>
                                        ))}
                                    </span>
                                    <span className="text-[11.5px] text-muted">
                                        {commit.authorName} · {relativeTime(commit.date)}
                                    </span>
                                </div>
                                {!isPushed(meta, commit.sha) && (
                                    <span
                                        className="h-1.5 w-1.5 shrink-0 rounded-full bg-accent"
                                        title="Only on this Mac — not pushed yet"
                                    />
                                )}
                                <span className="font-mono text-[11.5px] text-muted">
                                    {commit.sha.slice(0, 7)}
                                </span>
                            </div>
                        </li>
                    );
                })}
                {hasMore && (
                    <li className="p-3 text-center">
                        <Button onClick={() => void s.loadMore()} size="sm">
                            Load more
                        </Button>
                    </li>
                )}
                {!commits.length && (
                    <li className="p-8 text-center text-muted">
                        No commits yet. Make your first one from Changes.
                    </li>
                )}
            </ol>
            {menu && (
                <ContextMenu onClose={() => setMenu(null)} x={menu.x} y={menu.y}>
                    {commitActions(menu.commit, current)}
                </ContextMenu>
            )}
        </section>
    );
}

function Detail() {
    const s = useRepo();
    const { commits, detail, detailData, historyRef, meta, status } = s;
    const current = viewingCurrent(historyRef, status.branch.head);

    if (!detail) {
        return (
            <div className="flex flex-1 items-center justify-center text-muted">
                Select a commit to see its changes.
            </div>
        );
    }

    if (!detailData) {
        return <div className="flex flex-1 items-center justify-center text-muted">Loading…</div>;
    }

    let header: React.ReactNode;

    if (detail.kind === 'commit' && detailData.commit) {
        const c = detailData.commit;
        const pushed = isPushed(meta, c.sha);
        const later = commits.findIndex((other) => other.sha === c.sha);
        const safety = pushed
            ? 'Already on the remote. Renaming or removing it rewrites shared history and needs a force push. Revert is the safer choice.'
            : later <= 0
              ? 'Only on this Mac. Rename or remove it safely; you can undo from here.'
              : `Only on this Mac. Renaming or removing it rewrites ${later} later commit${later === 1 ? '' : 's'} safely; you can undo from here.`;

        header = (
            <>
                <div className="flex items-center gap-2">
                    <span className="font-mono text-[12.5px]">{c.sha.slice(0, 7)}</span>
                    <span
                        className={`rounded-full px-2 py-0.5 text-[11.5px] ${pushed ? 'bg-control text-muted' : 'bg-accent-bg text-accent'}`}
                    >
                        {pushed ? 'Pushed' : 'Not pushed'}
                    </span>
                    {c.parents.length > 1 && (
                        <span className="rounded-full bg-control px-2 py-0.5 text-[11.5px] text-muted">
                            Merge
                        </span>
                    )}
                    <span className="flex-1" />
                    <Button
                        onClick={() => void navigator.clipboard.writeText(c.sha)}
                        size="sm"
                        variant="ghost"
                    >
                        Copy SHA
                    </Button>
                </div>
                <h2 className="selectable m-0 text-[16px] leading-snug font-semibold">
                    {c.subject}
                </h2>
                {c.body && <p className="selectable m-0 whitespace-pre-wrap text-soft">{c.body}</p>}
                <span className="text-[12px] text-muted">
                    {c.authorName} &lt;{c.authorEmail}&gt; ·{' '}
                    {new Date(c.date * 1000).toLocaleString()}
                </span>
                <div className="flex flex-wrap gap-2">
                    {commitActions(c, current).map(([label, run]) => (
                        <Button
                            key={label}
                            onClick={run}
                            size="sm"
                            variant={label.startsWith('Remove') ? 'danger' : 'default'}
                        >
                            {label}
                        </Button>
                    ))}
                </div>
                {current && (
                    <span className={`text-[12px] ${pushed ? 'text-warn' : 'text-muted'}`}>
                        {safety}
                    </span>
                )}
            </>
        );
    } else if (detail.kind === 'stash') {
        const st = detail.stash;

        header = (
            <>
                <span className="font-mono text-[12px] text-muted">
                    stash@{`{${st.index}}`} · {relativeTime(st.date)}
                </span>
                <h2 className="m-0 text-[16px] font-semibold">{st.message}</h2>
                <div className="flex gap-2">
                    <Button
                        onClick={() =>
                            s.perform(
                                { pop: false, stash: st, type: 'stashApply' },
                                'Applying stash',
                            )
                        }
                        size="sm"
                    >
                        Apply
                    </Button>
                    <Button
                        onClick={() =>
                            s.perform({ pop: true, stash: st, type: 'stashApply' }, 'Popping stash')
                        }
                        size="sm"
                        variant="primary"
                    >
                        Pop
                    </Button>
                    <Button
                        onClick={() =>
                            s.perform({ stash: st, type: 'stashDrop' }, 'Dropping stash')
                        }
                        size="sm"
                        variant="danger"
                    >
                        Drop
                    </Button>
                </div>
            </>
        );
    } else {
        header = (
            <>
                <CombinedHeader shas={detail.kind === 'combined' ? detail.shas : []} />
            </>
        );
    }

    return (
        <section aria-label="Commit detail" className="flex min-w-0 flex-1 flex-col">
            <div className="flex max-h-[45%] shrink-0 flex-col gap-2.5 overflow-auto border-b border-line px-[18px] py-3.5">
                {header}
                <Files />
            </div>
            <DiffView diffs={detailData.diffs} />
        </section>
    );
}

function Files() {
    const files = useRepo((state) => state.detailData?.files ?? []);
    const [open, setOpen] = useState(false);

    if (!files.length) {
        return null;
    }

    return (
        <div className="flex flex-col gap-1">
            <button
                aria-expanded={open}
                className="flex items-center gap-1.5 text-left"
                onClick={() => setOpen((previous) => !previous)}
                type="button"
            >
                <Icon
                    className="text-muted"
                    name={open ? 'chevronDown' : 'chevronRight'}
                    size={11}
                />
                <SectionLabel>Files · {files.length}</SectionLabel>
            </button>
            {open &&
                files.map((file) => (
                    <div className="flex items-center gap-2 font-mono text-[12px]" key={file.path}>
                        <span className="flex-1 truncate" dir="rtl">
                            {file.path}
                        </span>
                        {file.additions == null ? (
                            <span className="text-muted">binary</span>
                        ) : (
                            <>
                                <span className="text-add">+{file.additions}</span>
                                <span className="text-del">−{file.deletions}</span>
                            </>
                        )}
                    </div>
                ))}
        </div>
    );
}

function Sidebar() {
    const { detail, historyRef, meta, perform, set, showDetail, status, viewHistory } = useRepo();
    const [showRemotes, setShowRemotes] = useState(false);
    const [menu, setMenu] = useState<{ branch: Branch; x: number; y: number } | null>(null);
    const local = meta.branches.filter((branch) => !branch.isRemote);
    const remote = meta.branches.filter((branch) => branch.isRemote);
    const isClean = !status.staged.length && !status.unstaged.length && !status.untracked.length;

    const branchRow = (branch: Branch) => {
        const viewed = branch.isCurrent ? historyRef === 'HEAD' : historyRef === branch.fullName;

        return (
            <button
                aria-current={viewed || undefined}
                className={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left font-mono text-[12px] ${viewed ? 'bg-selected' : 'hover:bg-hover'}`}
                key={branch.fullName}
                onClick={() => viewHistory(branch.isCurrent ? 'HEAD' : branch.fullName)}
                onContextMenu={(event) => {
                    event.preventDefault();
                    setMenu({ branch: branch, x: event.clientX, y: event.clientY });
                }}
                onDoubleClick={() =>
                    !branch.isCurrent &&
                    perform({ branch: branch.name, type: 'checkout' }, 'Switching branch')
                }
                title={branch.isCurrent ? 'Current branch' : 'Double-click to switch'}
                type="button"
            >
                <Icon
                    className={branch.isCurrent ? 'text-accent' : 'text-muted'}
                    name={branch.isCurrent ? 'check' : 'branch'}
                    size={12}
                />
                <span className={`flex-1 truncate ${branch.isCurrent ? 'font-semibold' : ''}`}>
                    {branch.name}
                </span>
                {branch.upstreamGone ? (
                    <span className="text-[11px] text-dim">gone</span>
                ) : (
                    (branch.ahead > 0 || branch.behind > 0) && (
                        <span className="text-[11px] text-muted">
                            {branch.ahead > 0 && `↑${branch.ahead}`}{' '}
                            {branch.behind > 0 && `↓${branch.behind}`}
                        </span>
                    )
                )}
            </button>
        );
    };

    return (
        <nav
            aria-label="Branches and stashes"
            className="flex h-full w-[250px] flex-col gap-5 overflow-auto border-r border-line bg-panel px-2 py-3.5"
        >
            <section className="flex flex-col gap-0.5">
                <SectionLabel className="px-2 pb-1.5">Branches</SectionLabel>
                {local.map(branchRow)}
                <button
                    className="flex items-center gap-2 px-2 py-1.5 text-left text-muted hover:text-fg"
                    onClick={() => set({ sheet: { from: null, kind: 'newBranch' } })}
                    type="button"
                >
                    <Icon name="plus" size={12} /> New branch…
                </button>
            </section>

            {remote.length > 0 && (
                <section className="flex flex-col gap-0.5">
                    <button
                        aria-expanded={showRemotes}
                        className="flex items-center gap-1.5 px-2 pb-1.5 text-left"
                        onClick={() => setShowRemotes((previous) => !previous)}
                        type="button"
                    >
                        <Icon
                            className="text-muted"
                            name={showRemotes ? 'chevronDown' : 'chevronRight'}
                            size={11}
                        />
                        <SectionLabel>Remote branches · {remote.length}</SectionLabel>
                    </button>
                    {showRemotes && remote.map(branchRow)}
                </section>
            )}

            <section className="flex flex-col gap-0.5">
                <SectionLabel className="px-2 pb-1.5">
                    Stashes {meta.stashes.length > 0 && `· ${meta.stashes.length}`}
                </SectionLabel>
                {meta.stashes.map((st) => {
                    const active = detail?.kind === 'stash' && detail.stash.sha === st.sha;

                    return (
                        <div
                            className={`flex flex-col gap-1 rounded-md px-2 py-1.5 ${active ? 'bg-selected' : 'hover:bg-hover'}`}
                            key={st.sha}
                        >
                            <button
                                className="flex flex-col gap-0.5 text-left"
                                onClick={() => showDetail({ kind: 'stash', stash: st })}
                                type="button"
                            >
                                <span className="truncate">{st.message}</span>
                                <span className="font-mono text-[11px] text-muted">
                                    stash@{'{'}
                                    {st.index}
                                    {'}'} · {relativeTime(st.date)}
                                </span>
                            </button>
                            <div className="flex gap-1.5">
                                <Button
                                    onClick={() =>
                                        perform(
                                            { pop: false, stash: st, type: 'stashApply' },
                                            'Applying stash',
                                        )
                                    }
                                    size="sm"
                                >
                                    Apply
                                </Button>
                                <Button
                                    onClick={() =>
                                        perform(
                                            { pop: true, stash: st, type: 'stashApply' },
                                            'Popping stash',
                                        )
                                    }
                                    size="sm"
                                >
                                    Pop
                                </Button>
                                <Button
                                    onClick={() =>
                                        perform({ stash: st, type: 'stashDrop' }, 'Dropping stash')
                                    }
                                    size="sm"
                                    variant="ghost"
                                >
                                    Drop
                                </Button>
                            </div>
                        </div>
                    );
                })}
                <button
                    className="flex items-center gap-2 px-2 py-1.5 text-left text-muted hover:text-fg disabled:opacity-40"
                    disabled={isClean}
                    onClick={() => set({ sheet: { kind: 'stash', paths: [] } })}
                    type="button"
                >
                    <Icon name="tray" size={12} /> Stash changes…
                </button>
            </section>

            {meta.tags.length > 0 && (
                <section className="flex flex-col gap-0.5">
                    <SectionLabel className="px-2 pb-1.5">Tags</SectionLabel>
                    {meta.tags.map((tag) => (
                        <div
                            className="group flex items-center gap-2 rounded-md px-2 py-1 font-mono text-[12px] hover:bg-hover"
                            key={tag.name}
                        >
                            <Icon className="text-muted" name="tag" size={12} />
                            <span className="flex-1 truncate">{tag.name}</span>
                            <button
                                aria-label={`Delete tag ${tag.name}`}
                                className="hidden text-muted group-hover:block hover:text-warn"
                                onClick={() =>
                                    perform({ name: tag.name, type: 'deleteTag' }, 'Deleting tag')
                                }
                                type="button"
                            >
                                <Icon name="x" size={12} />
                            </button>
                        </div>
                    ))}
                </section>
            )}

            {menu && (
                <ContextMenu onClose={() => setMenu(null)} x={menu.x} y={menu.y}>
                    {[
                        ...(!menu.branch.isCurrent
                            ? [
                                  [
                                      menu.branch.isRemote
                                          ? 'Check out as local branch'
                                          : `Switch to ${menu.branch.name}`,
                                      () =>
                                          perform(
                                              { branch: menu.branch.name, type: 'checkout' },
                                              'Switching branch',
                                          ),
                                  ] as const,
                              ]
                            : []),
                        [
                            'New branch from here…',
                            () => set({ sheet: { from: menu.branch.name, kind: 'newBranch' } }),
                        ] as const,
                        ...(!menu.branch.isRemote
                            ? [
                                  [
                                      'Rename…',
                                      () =>
                                          set({
                                              sheet: {
                                                  kind: 'renameBranch',
                                                  name: menu.branch.name,
                                                  upstream: menu.branch.upstream,
                                              },
                                          }),
                                  ] as const,
                              ]
                            : []),
                        ...(!menu.branch.isRemote && !menu.branch.isCurrent
                            ? [
                                  [
                                      'Delete…',
                                      () =>
                                          set({
                                              sheet: {
                                                  kind: 'deleteBranch',
                                                  name: menu.branch.name,
                                              },
                                          }),
                                  ] as const,
                              ]
                            : []),
                    ]}
                </ContextMenu>
            )}
        </nav>
    );
}

function viewingCurrent(historyRef: string, head: null | string) {
    return historyRef === 'HEAD' || historyRef === `refs/heads/${head}`;
}
