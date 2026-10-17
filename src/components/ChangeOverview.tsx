import { useEffect, useState } from 'react';

import type { Detail, FileDiff, ScopeSummary } from '../lib/types';

import { api, errorMessage } from '../lib/api';
import { useSettings } from '../lib/settings';
import { useRepo } from '../lib/store';
import { DiffView } from './DiffView';
import { Icon } from './Icon';
import { Button, Modal, relativeTime } from './ui';
import { DiffPanel } from './visual/DiffPanel';

/** The changed files grouped by top folder, each one's diff a click away. */
export function ChangedFiles({ diffs }: { diffs: FileDiff[] }) {
    const [open, setOpen] = useState<null | string>(null);
    const largest = Math.max(1, ...diffs.map((diff) => diff.additions + diff.deletions));
    const groups = new Map<string, FileDiff[]>();

    for (const diff of diffs) {
        const path = pathOf(diff);
        const group = path.includes('/') ? path.split('/')[0] : 'Top level';

        groups.set(group, [...(groups.get(group) ?? []), diff]);
    }

    if (!diffs.length) {
        return null;
    }

    return (
        <div className="flex flex-col overflow-hidden rounded-xl border border-line bg-panel">
            {[...groups].map(([group, files], index) => (
                <div className={index ? 'border-t border-line' : ''} key={group}>
                    <span className="block px-4 pt-2.5 pb-1 text-[11px] font-semibold tracking-[0.08em] text-muted uppercase">
                        {group}
                    </span>
                    {files.map((diff) => {
                        const path = pathOf(diff);
                        const slash = path.lastIndexOf('/');
                        const shown = open === path;
                        const share = ((diff.additions + diff.deletions) / largest) * 100;

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
    );
}

/** `%h %s` lines, each opening what that commit changed. */
export function Commits({ commits }: { commits: string[] }) {
    const [commit, setCommit] = useState<null | string>(null);

    return (
        <>
            <span className="text-[11px] font-semibold tracking-[0.08em] text-muted uppercase">
                Commits · {commits.length}
            </span>
            {commits.map((line) => {
                const [sha, ...subject] = line.split(' ');

                return (
                    <button
                        className="-mx-2 flex gap-2.5 rounded-md px-2 py-1 text-left leading-snug hover:bg-hover"
                        key={line}
                        onClick={() => setCommit(sha)}
                        title="See what this commit changes"
                        type="button"
                    >
                        <span className="shrink-0 font-mono text-[11.5px] text-dim">{sha}</span>
                        <span className="text-body">{subject.join(' ')}</span>
                    </button>
                );
            })}
            {commit && <CommitModal onClose={() => setCommit(null)} sha={commit} />}
        </>
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

export function ScopeLine({ scope }: { scope: ScopeSummary }) {
    return (
        <span className="flex flex-wrap items-center gap-x-2.5 gap-y-1 rounded-lg border border-line bg-panel px-3 py-2 font-mono text-[12px]">
            <span>{scope.label}</span>
            <span className="text-dim">
                {scope.commits > 0 && `${scope.commits} commit${scope.commits === 1 ? '' : 's'} · `}
                {scope.files} file{scope.files === 1 ? '' : 's'} ·{' '}
                <span className="text-add">+{scope.additions}</span>{' '}
                <span className="text-del">−{scope.deletions}</span>
            </span>
        </span>
    );
}

/** Elapsed time since `startedAt`, as m:ss, ticking every second. */
export function useClock(startedAt: number): string {
    const [now, setNow] = useState(() => Date.now() / 1000);

    useEffect(() => {
        const timer = window.setInterval(() => setNow(Date.now() / 1000), 1000);

        return () => window.clearInterval(timer);
    }, []);

    const elapsed = Math.max(0, Math.round(now - startedAt));

    return `${Math.floor(elapsed / 60)}:${String(elapsed % 60).padStart(2, '0')}`;
}

/** What an AI run would read right now: the branch against its base, else uncommitted work. */
export function useScope(): { nothing: null | string; scope: null | ScopeSummary } {
    const { root, status } = useRepo();
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

    return { nothing, scope };
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
