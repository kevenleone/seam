import type { Finding, ReviewState, ReviewStyle } from '../lib/types';

import { PROVIDER_LABELS, REVIEW_FOCUS, REVIEW_LEVELS, useAi } from '../lib/ai';
import { useSettingsModal } from '../lib/settingsModal';
import { branchLabel, useRepo } from '../lib/store';
import { ChangedFiles, Commits, ScopeLine, useClock, useScope } from './ChangeOverview';
import { FindingCard } from './FindingCard';
import { Icon } from './Icon';
import { Button } from './ui';

type RunningReview = Extract<ReviewState, { status: 'running' }>;

/** The header's half of a review: who is reading, for how long, and a way to stop it. */
export function ReviewRunStatus({ state }: { state: RunningReview }) {
    const { cancelReview } = useRepo();
    const clock = useClock(state.startedAt);

    return (
        <>
            <span className="flex min-w-0 items-center gap-2 text-[12.5px] text-soft" role="status">
                <span className="h-1.5 w-1.5 shrink-0 animate-pulse rounded-full bg-accent" />
                <span className="truncate">
                    {PROVIDER_LABELS[state.provider]} is reviewing{' '}
                    <span className="font-mono text-[12px]">{state.scope}</span>
                </span>
                <span className="font-mono text-[12px] text-muted">{clock}</span>
            </span>
            <span className="hidden text-[12px] text-dim xl:inline">
                Usually 1–3 minutes · keeps going if you close the window
            </span>
            <Button onClick={cancelReview} size="sm">
                Cancel
            </Button>
        </>
    );
}

/** While the model reviews: the change it is reading, and what it was asked to look for. */
export function WhileReviewing({ state }: { state: RunningReview }) {
    const { config } = useAi();
    const additions = state.diffs.reduce((sum, diff) => sum + diff.additions, 0);
    const deletions = state.diffs.reduce((sum, diff) => sum + diff.deletions, 0);

    return (
        <div className="mx-auto flex w-full max-w-[1080px] gap-8 overflow-auto px-8 py-8">
            <section className="flex min-w-0 flex-1 flex-col gap-3.5">
                <div className="flex flex-col gap-1">
                    <h2 className="m-0 text-[17px] font-semibold">
                        While it reviews, here is what it is reading
                    </h2>
                    <span className="text-muted">
                        {state.diffs.length} file{state.diffs.length === 1 ? '' : 's'} ·{' '}
                        <span className="text-add">+{additions}</span>{' '}
                        <span className="text-del">−{deletions}</span>. Comments land on the lines
                        they are about; open a file to read it first.
                    </span>
                </div>
                <ChangedFiles diffs={state.diffs} />
            </section>
            <aside className="flex w-[300px] shrink-0 flex-col gap-4">
                {config && <StyleSummary style={config.review} />}
                {state.commits.length > 0 && (
                    <div className="flex flex-col gap-2.5">
                        <Commits commits={state.commits} />
                    </div>
                )}
            </aside>
        </div>
    );
}

/** What the review is told to look for, from Settings › AI. */
function StyleSummary({ style }: { style: ReviewStyle }) {
    const focus = REVIEW_FOCUS.filter((area) => style.focus.includes(area.id));
    const shown = focus.length ? focus : REVIEW_FOCUS.slice(0, 4);

    return (
        <div className="flex flex-col gap-2.5 rounded-xl border border-line bg-panel p-4">
            <div className="flex items-center justify-between gap-2">
                <span className="text-[11px] font-semibold tracking-[0.08em] text-muted uppercase">
                    Looking for
                </span>
                <button
                    className="text-[12px] text-accent-strong hover:underline"
                    onClick={() => useSettingsModal.getState().show('ai')}
                    type="button"
                >
                    Change
                </button>
            </div>
            <ul className="m-0 flex list-none flex-col gap-1.5 p-0">
                {shown.map((area) => (
                    <li className="flex gap-2 leading-snug" key={area.id}>
                        <Icon className="mt-0.5 shrink-0 text-accent" name="check" size={12} />
                        <span className="flex flex-col">
                            <span>{area.title}</span>
                            <span className="text-[12px] text-muted">{area.desc}</span>
                        </span>
                    </li>
                ))}
            </ul>
            <span className="text-[12px] text-muted">
                {REVIEW_LEVELS[style.level]} · in {style.language.trim() || 'English'}
            </span>
        </div>
    );
}

const EXAMPLE: Finding = {
    body: 'A max of 0 now means unlimited, but allow(0, 0) used to return true and now returns false.',
    file: 'src/limit.js',
    id: 0,
    line: 3,
    severity: 'risk',
    suggestion: 'return max === 0 || hits <= max;',
    title: 'Unlimited quota rejects the first request',
};

/** Before the first review: what it will read, how picky it is, and what comes back. */
export function ReviewEmpty() {
    const { startReview, status } = useRepo();
    const { config } = useAi();
    const { nothing, scope } = useScope();

    return (
        <div className="flex min-h-full items-center justify-center gap-16 overflow-auto px-12 py-10">
            <section className="flex w-[420px] flex-col gap-4">
                <span className="text-[11px] font-semibold tracking-[0.12em] text-muted uppercase">
                    AI review
                </span>
                <h2 className="m-0 text-[24px] leading-tight font-semibold">
                    Catch what should stop {branchLabel(status)} from merging
                </h2>
                <p className="m-0 leading-relaxed text-body">
                    The model you chose in Settings reads the branch against its base, with the code
                    around it, and leaves its comments on the lines they are about. Nothing in the
                    repository changes.
                </p>
                {scope && <ScopeLine scope={scope} />}
                {nothing && (
                    <span className="rounded-lg bg-warn-bg px-3 py-2 text-[12.5px] text-warn">
                        {nothing}
                    </span>
                )}
                <div className="flex items-center gap-3">
                    <Button
                        disabled={!!nothing}
                        onClick={() => startReview()}
                        size="lg"
                        variant="primary"
                    >
                        <Icon name="sparkle" size={13} /> Review {branchLabel(status)}
                    </Button>
                    <span className="text-[12px] text-dim">⇧⌘R · about 1–3 minutes</span>
                </div>
                {config && <StyleSummary style={config.review} />}
            </section>
            <figure
                aria-label="An example comment"
                className="m-0 hidden w-[480px] flex-col gap-2 lg:flex"
            >
                <div className="pointer-events-none overflow-hidden rounded-2xl border border-line bg-app shadow-[0_24px_60px_-36px_var(--shadow)]">
                    <div className="border-b border-line bg-panel px-4 py-2 font-mono text-[12px]">
                        src/limit.js
                    </div>
                    <div className="py-1.5 font-mono text-[12px] leading-[1.7]">
                        <div className="flex">
                            <span className="w-10 pr-2 text-right text-dim">1</span>
                            <span className="pl-4 text-body">
                                export function allow(hits, max) {'{'}
                            </span>
                        </div>
                        <div className="flex bg-add-bg">
                            <span className="w-10 pr-2 text-right text-add-num">2</span>
                            <span className="pl-1 text-add">+</span>
                            <span className="pl-2 text-body"> // max 0 means unlimited</span>
                        </div>
                        <div className="flex bg-add-bg">
                            <span className="w-10 pr-2 text-right text-add-num">3</span>
                            <span className="pl-1 text-add">+</span>
                            <span className="pl-2 text-body">
                                {' '}
                                return hits &lt; max || max == 0;
                            </span>
                        </div>
                    </div>
                    <div className="pb-3">
                        <FindingCard finding={EXAMPLE} inline onDismiss={() => undefined} />
                    </div>
                </div>
                <figcaption className="text-center text-[12px] text-dim">
                    An example: a comment on the line it is about
                </figcaption>
            </figure>
        </div>
    );
}
