import type { ReactNode } from 'react';

import type { BranchStatus } from '../lib/types';

import { useRepo } from '../lib/store';
import { Commits, useScope } from './ChangeOverview';
import { relativeTime } from './ui';

/** The Changes tab with nothing to commit: where the branch stands and what usually comes next. */
export function CleanTree() {
    const s = useRepo();
    const { scope } = useScope();
    const { branch } = s.status;
    const last = s.lastCommit;
    const onBranch = scope && scope.commits > 0 && s.historyRef === 'HEAD';
    const branchCommits = onBranch
        ? s.commits
              .slice(0, Math.min(scope.commits, 5))
              .map((commit) => `${commit.sha.slice(0, 7)} ${commit.subject}`)
        : [];

    return (
        <div className="flex flex-1 justify-center overflow-auto px-8 py-14">
            <div className="flex w-full max-w-[680px] flex-col gap-7">
                <section className="flex flex-col gap-2.5">
                    <span className="text-[11px] font-semibold tracking-[0.12em] text-muted uppercase">
                        Nothing to commit
                    </span>
                    <h2 className="m-0 text-[24px] leading-tight font-semibold">
                        {standing(branch)}
                    </h2>
                    <p className="m-0 leading-relaxed text-body">
                        {onBranch && s.baseBranch && (
                            <>
                                {plural(scope.commits, 'commit')} beyond {s.baseBranch}, across{' '}
                                {plural(scope.files, 'file')}{' '}
                                <span className="text-add">+{scope.additions}</span>{' '}
                                <span className="text-del">−{scope.deletions}</span>.{' '}
                            </>
                        )}
                        {last && (
                            <>
                                The last commit was {relativeTime(last.date)}:{' '}
                                <span className="font-mono text-[12px] text-dim">
                                    {last.sha.slice(0, 7)}
                                </span>{' '}
                                {last.subject}
                            </>
                        )}
                    </p>
                </section>
                <section className="grid grid-cols-2 gap-2.5">
                    {branch.head && (branch.ahead > 0 || !branch.upstream) && (
                        <Action
                            detail={
                                branch.upstream ? `To ${branch.upstream}` : 'Publish it to a remote'
                            }
                            keys="⌘P"
                            onClick={() => s.push()}
                            title={
                                branch.upstream
                                    ? `Push ${plural(branch.ahead, 'commit')}`
                                    : 'Push the branch'
                            }
                        />
                    )}
                    {branch.behind > 0 && (
                        <Action
                            detail={`From ${branch.upstream}`}
                            keys="⇧⌘P"
                            onClick={() => s.pull()}
                            title={`Pull ${plural(branch.behind, 'commit')}`}
                        />
                    )}
                    {scope && (
                        <Action
                            detail={
                                <>
                                    {plural(scope.files, 'file')} ·{' '}
                                    <span className="text-add">+{scope.additions}</span>{' '}
                                    <span className="text-del">−{scope.deletions}</span>
                                </>
                            }
                            keys="⌘3"
                            onClick={() => s.set({ tab: 'review' })}
                            title="Review the branch"
                        />
                    )}
                    {scope && (
                        <Action
                            detail="A picture of the change before you share it"
                            keys="⇧⌘V"
                            onClick={() => s.set({ tab: 'visual', visualKind: 'recap' })}
                            title="Draw the recap"
                        />
                    )}
                    <Action
                        detail="Start from here, or switch with ⌘K"
                        keys="⇧⌘N"
                        onClick={() => s.set({ sheet: { from: null, kind: 'newBranch' } })}
                        title="New branch"
                    />
                </section>
                {branchCommits.length > 0 && (
                    <section className="flex flex-col gap-2">
                        <Commits commits={branchCommits} />
                    </section>
                )}
            </div>
        </div>
    );
}

function Action({
    detail,
    keys,
    onClick,
    title,
}: {
    detail: ReactNode;
    keys: string;
    onClick(): void;
    title: string;
}) {
    return (
        <button
            className="flex flex-col items-start gap-1 rounded-[10px] border border-control-line bg-control px-4 py-3.5 text-left hover:bg-hover"
            onClick={onClick}
            type="button"
        >
            <span className="flex w-full justify-between font-semibold">
                {title}
                <span className="font-normal text-dim">{keys}</span>
            </span>
            <span className="text-[12px] text-muted">{detail}</span>
        </button>
    );
}

function plural(count: number, noun: string): string {
    return `${count} ${noun}${count === 1 ? '' : 's'}`;
}

function standing(branch: BranchStatus): string {
    if (!branch.head) {
        return `Detached at ${branch.oid?.slice(0, 7) ?? 'a commit'}`;
    }

    if (!branch.upstream) {
        return `${branch.head} isn't on a remote yet`;
    }

    if (branch.ahead > 0 && branch.behind > 0) {
        return `${branch.head} and ${branch.upstream} have diverged`;
    }

    if (branch.ahead > 0) {
        return `${branch.head} is ${plural(branch.ahead, 'commit')} ahead of ${branch.upstream}`;
    }

    if (branch.behind > 0) {
        return `${branch.head} is ${plural(branch.behind, 'commit')} behind ${branch.upstream}`;
    }

    return `${branch.head} is up to date with ${branch.upstream}`;
}
