import type { VisualState } from '../../lib/types';

import { PROVIDER_LABELS } from '../../lib/ai';
import { branchLabel, useRepo } from '../../lib/store';
import { ChangedFiles, Commits, ScopeLine, useClock, useScope } from '../ChangeOverview';
import { Icon } from '../Icon';
import { Button } from '../ui';

type RunningState = Extract<VisualState, { status: 'running' }>;

/** Before the first recap: what it would cover, one button, and what comes back. */
export function RecapEmpty() {
    const { startVisual, status } = useRepo();
    const { nothing, scope } = useScope();

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
                {scope && <ScopeLine scope={scope} />}
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
    const additions = state.diffs.reduce((sum, diff) => sum + diff.additions, 0);
    const deletions = state.diffs.reduce((sum, diff) => sum + diff.deletions, 0);
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
                <ChangedFiles diffs={state.diffs} />
            </section>
            {state.commits.length > 0 && (
                <aside className="flex w-[280px] shrink-0 flex-col gap-2.5">
                    <Commits commits={state.commits} />
                </aside>
            )}
        </div>
    );
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
