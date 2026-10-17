import { useEffect, useState } from 'react';

import type { Detail, FileDiff, ScopeSummary, VisualState } from '../../lib/types';

import { PROVIDER_LABELS } from '../../lib/ai';
import { api, errorMessage } from '../../lib/api';
import { useSettings } from '../../lib/settings';
import { branchLabel, useRepo } from '../../lib/store';
import { installSketchStyle } from '../../lib/visual';
import { DiffView } from '../DiffView';
import { Icon } from '../Icon';
import { Button, Modal, relativeTime } from '../ui';
import { DiffPanel } from './DiffPanel';

installSketchStyle();

type RunningState = Extract<VisualState, { status: 'running' }>;

/** Before the first recap: what it would cover, one button, and what comes back. */
export function RecapEmpty() {
    const { root, startVisual, status } = useRepo();
    const [scope, setScope] = useState<null | ScopeSummary>(null);
    const [nothing, setNothing] = useState<null | string>(null);

    useEffect(() => {
        api.visualScope(root).then(
            (summary) => {
                setScope(summary);
                setNothing(null);
            },
            (error) => setNothing(errorMessage(error)),
        );
    }, [root, status.branch.oid, status.staged.length, status.unstaged.length]);

    return (
        <div className="flex min-h-full items-center justify-center gap-16 px-12 py-10">
            <section className="flex w-[420px] flex-col gap-4">
                <span className="text-[11px] font-semibold tracking-[0.12em] text-muted uppercase">
                    Visual recap
                </span>
                <h2 className="m-0 text-[24px] leading-tight font-semibold">
                    See what {branchLabel(status)} changes before you read the diff
                </h2>
                <p className="m-0 leading-relaxed text-body">
                    The agent reads the branch against its base and draws a map of it: the screens
                    it touches before and after, how the pieces connect, the decisions behind it,
                    the diffs worth a close look and how to test it.
                </p>
                {scope && (
                    <span className="flex flex-wrap items-center gap-x-2.5 gap-y-1 rounded-lg border border-line bg-panel px-3 py-2 font-mono text-[12px]">
                        <span>{scope.label}</span>
                        <span className="text-dim">
                            {scope.commits > 0 &&
                                `${scope.commits} commit${scope.commits === 1 ? '' : 's'} · `}
                            {scope.files} file{scope.files === 1 ? '' : 's'} ·{' '}
                            <span className="text-add">+{scope.additions}</span>{' '}
                            <span className="text-del">−{scope.deletions}</span>
                        </span>
                    </span>
                )}
                {nothing && (
                    <span className="rounded-lg bg-warn-bg px-3 py-2 text-[12.5px] text-warn">
                        {nothing}
                    </span>
                )}
                <div className="flex items-center gap-3">
                    <Button
                        disabled={!!nothing}
                        onClick={() => startVisual('recap')}
                        size="lg"
                        variant="primary"
                    >
                        <Icon name="sparkle" size={13} /> Draw the recap
                    </Button>
                    <span className="text-[12px] text-dim">⇧⌘V · about 2–5 minutes</span>
                </div>
                <span className="text-[12px] text-dim">
                    Exports to a web page, PDF, Markdown or Excalidraw.
                </span>
            </section>
            <RecapExample />
        </div>
    );
}

/** A bar sliding under the header while a run is going. */
export function RunBar() {
    return (
        <div aria-hidden className="relative h-0.5 shrink-0 overflow-hidden bg-line">
            <div className="vz-run-bar absolute inset-y-0 left-0 w-1/4 bg-accent" />
        </div>
    );
}

/** The header's half of a run: who is drawing, for how long, and a way to stop it. */
export function RunStatus({ state }: { state: RunningState }) {
    const { cancelVisual } = useRepo();
    const clock = useClock(state.startedAt);

    return (
        <>
            <span className="flex min-w-0 items-center gap-2 text-[12.5px] text-soft" role="status">
                <span className="h-1.5 w-1.5 shrink-0 animate-pulse rounded-full bg-accent" />
                <span className="truncate">
                    {PROVIDER_LABELS[state.provider]} is drawing{' '}
                    {state.kind === 'plan' ? 'the plan' : 'the recap'}
                </span>
                <span className="font-mono text-[12px] text-muted">{clock}</span>
            </span>
            <span className="hidden text-[12px] text-dim xl:inline">
                Usually 2–5 minutes · keeps going if you close the window
            </span>
            <Button onClick={() => cancelVisual(state.kind)} size="sm">
                Cancel
            </Button>
        </>
    );
}

/** While the agent draws: the change it is reading, file by file, each diff a click away. */
export function WhileDrawing({ state }: { state: RunningState }) {
    const [open, setOpen] = useState<null | string>(null);
    const [commit, setCommit] = useState<null | string>(null);
    const additions = state.diffs.reduce((sum, diff) => sum + diff.additions, 0);
    const deletions = state.diffs.reduce((sum, diff) => sum + diff.deletions, 0);
    const largest = Math.max(1, ...state.diffs.map((diff) => diff.additions + diff.deletions));
    const groups = new Map<string, FileDiff[]>();

    for (const diff of state.diffs) {
        const path = pathOf(diff);
        const group = path.includes('/') ? path.split('/')[0] : 'Top level';

        groups.set(group, [...(groups.get(group) ?? []), diff]);
    }

    const plan = state.kind === 'plan';

    return (
        <div className="mx-auto flex w-full max-w-[1080px] gap-8 px-8 py-8">
            <section className="flex min-w-0 flex-1 flex-col gap-3.5">
                <div className="flex flex-col gap-1">
                    <h2 className="m-0 text-[17px] font-semibold">
                        {plan
                            ? 'While it plans, here is what it was asked'
                            : 'While it draws, here is what it is reading'}
                    </h2>
                    {plan ? (
                        <p className="selectable m-0 leading-normal whitespace-pre-wrap text-body">
                            {state.prompt}
                        </p>
                    ) : (
                        <span className="text-muted">
                            {state.diffs.length} file{state.diffs.length === 1 ? '' : 's'} ·{' '}
                            <span className="text-add">+{additions}</span>{' '}
                            <span className="text-del">−{deletions}</span> in{' '}
                            <span className="font-mono text-[12px]">{state.scope}</span>. Open a
                            file to read its diff.
                        </span>
                    )}
                </div>
                {plan && state.diffs.length > 0 && (
                    <span className="text-[12px] text-muted">
                        It builds on the uncommitted work below.
                    </span>
                )}
                {state.diffs.length > 0 && (
                    <div className="flex flex-col overflow-hidden rounded-xl border border-line bg-panel">
                        {[...groups].map(([group, diffs], index) => (
                            <div className={index ? 'border-t border-line' : ''} key={group}>
                                <span className="block px-4 pt-2.5 pb-1 text-[11px] font-semibold tracking-[0.08em] text-muted uppercase">
                                    {group}
                                </span>
                                {diffs.map((diff) => {
                                    const path = pathOf(diff);
                                    const slash = path.lastIndexOf('/');
                                    const shown = open === path;
                                    const share =
                                        ((diff.additions + diff.deletions) / largest) * 100;

                                    return (
                                        <div key={path}>
                                            <button
                                                aria-expanded={shown}
                                                className={`flex w-full items-center gap-3 px-4 py-1.5 text-left font-mono text-[12px] hover:bg-hover ${shown ? 'bg-hover' : ''}`}
                                                onClick={() => setOpen(shown ? null : path)}
                                                type="button"
                                            >
                                                <Icon
                                                    className="shrink-0 text-dim"
                                                    name={shown ? 'chevronDown' : 'chevronRight'}
                                                    size={11}
                                                />
                                                <span className="min-w-0 flex-1 truncate">
                                                    <span className="text-muted">
                                                        {path.slice(0, slash + 1)}
                                                    </span>
                                                    {path.slice(slash + 1)}
                                                </span>
                                                <span className="w-12 shrink-0 text-right text-add">
                                                    +{diff.additions}
                                                </span>
                                                <span className="w-10 shrink-0 text-right text-del">
                                                    −{diff.deletions}
                                                </span>
                                                <span className="h-1.5 w-24 shrink-0 overflow-hidden rounded-full bg-line">
                                                    <span
                                                        className="block h-full rounded-full bg-add"
                                                        style={{ width: `${Math.max(4, share)}%` }}
                                                    />
                                                </span>
                                            </button>
                                            {shown && (
                                                <div className="border-y border-line">
                                                    <DiffPanel diff={diff} />
                                                </div>
                                            )}
                                        </div>
                                    );
                                })}
                            </div>
                        ))}
                    </div>
                )}
            </section>
            {state.commits.length > 0 && (
                <aside className="flex w-[280px] shrink-0 flex-col gap-2.5">
                    <span className="text-[11px] font-semibold tracking-[0.08em] text-muted uppercase">
                        Commits · {state.commits.length}
                    </span>
                    {state.commits.map((commit) => {
                        const [sha, ...subject] = commit.split(' ');

                        return (
                            <button
                                className="-mx-2 flex gap-2.5 rounded-md px-2 py-1 text-left leading-snug hover:bg-hover"
                                key={commit}
                                onClick={() => setCommit(sha)}
                                title="See what this commit changes"
                                type="button"
                            >
                                <span className="shrink-0 font-mono text-[11.5px] text-dim">
                                    {sha}
                                </span>
                                <span className="text-body">{subject.join(' ')}</span>
                            </button>
                        );
                    })}
                </aside>
            )}
            {commit && <CommitModal onClose={() => setCommit(null)} sha={commit} />}
        </div>
    );
}

/** One commit of the branch: its message and everything it changed. */
function CommitModal({ onClose, sha }: { onClose(): void; sha: string }) {
    const { root } = useRepo();
    const { ignoreWhitespace } = useSettings();
    const [detail, setDetail] = useState<Detail | null>(null);
    const [failure, setFailure] = useState<null | string>(null);

    useEffect(() => {
        api.commitDetail(root, sha, ignoreWhitespace).then(setDetail, (error) =>
            setFailure(errorMessage(error)),
        );
    }, [ignoreWhitespace, root, sha]);

    const info = detail?.commit;
    const additions = detail?.files.reduce((sum, file) => sum + (file.additions ?? 0), 0) ?? 0;
    const deletions = detail?.files.reduce((sum, file) => sum + (file.deletions ?? 0), 0) ?? 0;

    return (
        <Modal onClose={onClose} title={info?.subject ?? sha} width={1100}>
            <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[12px] text-muted">
                <span className="font-mono">{info?.sha.slice(0, 10) ?? sha}</span>
                {info && (
                    <span>
                        {info.authorName} · {relativeTime(info.date)}
                    </span>
                )}
                {detail && (
                    <span className="font-mono">
                        {detail.files.length} file{detail.files.length === 1 ? '' : 's'} ·{' '}
                        <span className="text-add">+{additions}</span>{' '}
                        <span className="text-del">−{deletions}</span>
                    </span>
                )}
            </div>
            {info?.body && (
                <p className="selectable m-0 max-h-28 overflow-auto leading-normal whitespace-pre-wrap text-body">
                    {info.body}
                </p>
            )}
            <div className="flex h-[60vh] flex-col overflow-hidden rounded-lg border border-line bg-app">
                {failure ? (
                    <span className="m-auto text-del">{failure}</span>
                ) : detail ? (
                    <DiffView diffs={detail.diffs} />
                ) : (
                    <span className="m-auto text-muted">Loading…</span>
                )}
            </div>
            <div className="flex justify-end">
                <Button onClick={onClose}>Close</Button>
            </div>
        </Modal>
    );
}

function pathOf(diff: FileDiff): string {
    return diff.newPath ?? diff.oldPath ?? '';
}

/** A small, fixed picture of what a recap holds; not drawn from this repository. */
function RecapExample() {
    return (
        <figure
            aria-label="An example recap"
            className="m-0 hidden w-[520px] flex-col gap-2 lg:flex"
        >
            <div className="overflow-hidden rounded-2xl border border-line bg-panel shadow-[0_24px_60px_-36px_var(--shadow)]">
                <div className="flex flex-col gap-1.5 px-5 pt-4 pb-3">
                    <span className="text-[9.5px] font-semibold tracking-[0.12em] text-dim uppercase">
                        Visual recap
                    </span>
                    <span className="text-[15px] font-semibold">
                        Sign-up form now asks for a password and the terms
                    </span>
                </div>
                <div className="flex h-[168px] items-center justify-center gap-10 bg-canvas bg-[radial-gradient(var(--line)_1px,transparent_1px)] bg-size-[14px_14px]">
                    <div className="wf-root h-[112px]! w-[170px]! rounded-lg border-[1.5px] border-control-line p-3!">
                        <span className="wf-input">email</span>
                        <span className="wf-button">Sign up</span>
                    </div>
                    <span className="text-[20px] text-accent">→</span>
                    <div className="wf-root h-[112px]! w-[170px]! gap-1.5! rounded-lg border-[1.5px] border-control-line p-3!">
                        <span className="wf-input">email</span>
                        <span className="wf-input wf-changed">password</span>
                        <span className="wf-row wf-changed">
                            <span className="wf-checkbox" /> I accept the terms
                        </span>
                    </div>
                </div>
                <div className="grid grid-cols-3 gap-2.5 px-5 pt-3.5 pb-4 text-[11.5px] text-soft">
                    {['Architecture', 'Decisions', 'Key diffs'].map((label) => (
                        <span
                            className="rounded-lg border border-line bg-app px-2.5 py-2"
                            key={label}
                        >
                            {label}
                        </span>
                    ))}
                </div>
            </div>
            <figcaption className="text-center text-[12px] text-dim">
                An example of what comes back for a small change
            </figcaption>
        </figure>
    );
}

function useClock(startedAt: number): string {
    const [now, setNow] = useState(() => Date.now() / 1000);

    useEffect(() => {
        const timer = window.setInterval(() => setNow(Date.now() / 1000), 1000);

        return () => window.clearInterval(timer);
    }, []);

    const elapsed = Math.max(0, Math.round(now - startedAt));

    return `${Math.floor(elapsed / 60)}:${String(elapsed % 60).padStart(2, '0')}`;
}
