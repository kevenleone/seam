import { type ReactNode, useEffect, useRef, useState } from 'react';

import type {
    Drift,
    Facts,
    Overview,
    OverviewSection,
    OverviewState,
    VisualBlock,
} from '../lib/types';

import { RunBar, useClock } from '../components/ChangeOverview';
import { Icon } from '../components/Icon';
import { ProviderSelect } from '../components/ProviderSelect';
import { Button, relativeTime } from '../components/ui';
import { CodeCard, FileTreeCard } from '../components/visual/Blocks';
import { Diagram } from '../components/visual/Diagram';
import { Prose } from '../components/visual/Markdown';
import { Visual } from '../components/visual/mode';
import { PROVIDER_LABELS } from '../lib/ai';
import { useSettings } from '../lib/settings';
import { useSettingsModal } from '../lib/settingsModal';
import { useRepo } from '../lib/store';
import { useHighlighter } from '../lib/useHighlighter';

type RunningState = Extract<OverviewState, { status: 'running' }>;

const LANGUAGE_COLORS = [
    'var(--syn-keyword)',
    'var(--syn-string)',
    'var(--syn-function)',
    'var(--syn-type)',
    'var(--syn-number)',
    'var(--syn-attr)',
    'var(--dim)',
];

const CARD = 'flex min-w-0 flex-col gap-3 rounded-xl border border-line bg-panel p-4';

export function OverviewView() {
    const state = useRepo((store) => store.overview);
    const facts = useRepo((store) => store.facts);
    const drift = useRepo((store) => store.drift);
    const head = useRepo((store) => store.status.branch.oid);
    const { syntaxHighlight } = useSettings();
    const highlighter = useHighlighter(syntaxHighlight);
    const shown = shownOverview(state);

    useEffect(() => {
        void useRepo.getState().loadOverview();
    }, [head]);

    return (
        <section aria-label="Overview" className="flex min-w-0 flex-1 flex-col">
            <OverviewHeader drift={drift} shown={shown} state={state} />
            {state.status === 'running' && <RunBar />}
            <div className="min-h-0 flex-1 overflow-auto">
                {state.status === 'failed' && <Failure state={state} />}
                <Visual.Provider value={{ highlighter, mode: 'app' }}>
                    <div className="mx-auto flex w-full max-w-[1080px] flex-col gap-6 px-8 pt-8 pb-16">
                        <Intro facts={facts} shown={shown} />
                        {facts && <FactCards facts={facts} />}
                        {shown ? (
                            <Sections drift={drift} overview={shown} state={state} />
                        ) : (
                            <Invitation state={state} />
                        )}
                    </div>
                </Visual.Provider>
            </div>
        </section>
    );
}

function Activity({ facts }: { facts: Facts }) {
    const peak = Math.max(1, ...facts.weekly);
    const width = 240;
    const height = 48;
    const step = facts.weekly.length > 1 ? width / (facts.weekly.length - 1) : width;
    const points = facts.weekly.map(
        (count, index) => [index * step, height - 4 - (count / peak) * (height - 10)] as const,
    );
    const line = points.map(([left, top], index) => `${index ? 'L' : 'M'}${left},${top}`).join(' ');
    const last = points.at(-1);
    const thisWeek = facts.weekly.at(-1) ?? 0;

    return (
        <section className={CARD}>
            <CardTitle source="git log">Activity</CardTitle>
            <div className="grid grid-cols-3 gap-3">
                <Stat label="commits" value={facts.commits.toLocaleString('en')} />
                <Stat label="latest tag" value={facts.latestTag ?? '—'} />
                <Stat
                    label={facts.contributors === 1 ? 'contributor' : 'contributors'}
                    value={String(facts.contributors)}
                />
            </div>
            {facts.commits > 0 && (
                <figure className="m-0 flex flex-col gap-1">
                    <svg
                        aria-label={`Commits per week over the last ${facts.weekly.length} weeks`}
                        className="block h-12 w-full"
                        preserveAspectRatio="none"
                        role="img"
                        viewBox={`0 0 ${width} ${height}`}
                    >
                        <line
                            stroke="var(--line)"
                            x1="0"
                            x2={width}
                            y1={height - 1}
                            y2={height - 1}
                        />
                        <path
                            d={`${line} L${width},${height} L0,${height} Z`}
                            fill="var(--add-bg)"
                        />
                        <path
                            d={line}
                            fill="none"
                            stroke="var(--add)"
                            strokeWidth="1.5"
                            vectorEffect="non-scaling-stroke"
                        />
                        {last && <circle cx={last[0]} cy={last[1]} fill="var(--add)" r="2.5" />}
                    </svg>
                    <figcaption className="flex justify-between text-[11.5px] text-dim">
                        <span>{facts.weekly.length} weeks ago</span>
                        <span>
                            this week · {thisWeek} commit{thisWeek === 1 ? '' : 's'}
                        </span>
                    </figcaption>
                </figure>
            )}
        </section>
    );
}

function CardTitle({ children, source }: { children: string; source?: string }) {
    return (
        <h3 className="m-0 flex items-center justify-between gap-2 text-[11px] font-semibold tracking-[0.1em] text-muted uppercase">
            {children}
            {source && (
                <span className="font-mono text-[10.5px] font-normal tracking-normal text-dim normal-case">
                    {source}
                </span>
            )}
        </h3>
    );
}

function FactCards({ facts }: { facts: Facts }) {
    return (
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
            <Stack facts={facts} />
            <Activity facts={facts} />
            {facts.hotPaths.length > 0 && <HotPaths facts={facts} />}
            {facts.scripts.length > 0 && <Scripts facts={facts} />}
        </div>
    );
}

function Failure({ state }: { state: Extract<OverviewState, { status: 'failed' }> }) {
    const { startOverview } = useRepo();

    return (
        <div
            className="flex items-start gap-2.5 border-b border-del-line bg-del-bg px-[18px] py-2.5"
            role="alert"
        >
            <Icon className="mt-0.5 shrink-0 text-del" name="warning" size={14} />
            <pre className="selectable m-0 flex-1 font-sans whitespace-pre-wrap text-body">
                {state.message}
            </pre>
            <Button
                onClick={() => useSettingsModal.getState().show('ai')}
                size="sm"
                variant="ghost"
            >
                AI settings
            </Button>
            <Button onClick={() => startOverview(undefined, state.provider)} size="sm">
                Try again
            </Button>
        </div>
    );
}

/** Where it stands against HEAD, measured with git alone. */
function Freshness({ drift, shown }: { drift: Drift | null; shown: null | Overview }) {
    if (!shown || !drift) {
        return <span className="text-[12px] text-muted">Read from git and the manifests</span>;
    }

    const written = `${relativeTime(shown.createdAt)} by ${PROVIDER_LABELS[shown.provider]}`;
    const titles = shown.sections
        .filter((section) => drift.sections.includes(section.id))
        .map((section) => section.title);

    if (drift.missing) {
        return (
            <Pill tone="warn">Written {written}, at a commit this repository no longer has</Pill>
        );
    }

    if (titles.length) {
        return (
            <Pill tone="warn">
                {drift.commits} commit{drift.commits === 1 ? '' : 's'} since · {titles.join(', ')}{' '}
                changed
            </Pill>
        );
    }

    const commits = `${drift.commits} commit${drift.commits === 1 ? '' : 's'} since`;

    return (
        <Pill tone="fresh">
            {drift.minor.length
                ? `Up to date · ${commits}, small edits only`
                : drift.commits
                  ? `Up to date · ${commits}, none in what it covers`
                  : `Up to date · written ${written}`}
        </Pill>
    );
}

function HistoryMenu({ disabled, shownId }: { disabled: boolean; shownId: null | string }) {
    const entries = useRepo((store) => store.overviews);
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

    if (entries.length < 2) {
        return null;
    }

    return (
        <div className="relative" ref={menu}>
            <Button
                aria-expanded={open}
                aria-haspopup="menu"
                disabled={disabled}
                onClick={() => setOpen(!open)}
                size="sm"
                title="Earlier overviews, to see how the project changed"
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
                    {entries.map((entry) => (
                        <button
                            className={`flex flex-col gap-0.5 rounded-md px-2.5 py-1.5 text-left hover:bg-hover ${entry.id === shownId ? 'bg-selected' : ''}`}
                            key={entry.id}
                            onClick={() => {
                                setOpen(false);
                                useRepo.getState().openOverview(entry.id);
                            }}
                            role="menuitem"
                            type="button"
                        >
                            <span className="line-clamp-2 leading-snug">{entry.purpose}</span>
                            <span className="flex items-center gap-1.5 font-mono text-[11.5px] text-muted">
                                {entry.headSha?.slice(0, 7)}
                                {entry.branch && <span className="truncate">{entry.branch}</span>}
                                <span className="shrink-0 font-sans">
                                    · {relativeTime(entry.createdAt)}
                                </span>
                            </span>
                        </button>
                    ))}
                </div>
            )}
        </div>
    );
}

function HotPaths({ facts }: { facts: Facts }) {
    const peak = Math.max(1, ...facts.hotPaths.map((hot) => hot.changes));

    return (
        <section className={CARD}>
            <CardTitle source="last 30 days">Where work happens</CardTitle>
            <div className="flex flex-col gap-2">
                {facts.hotPaths.map((hot) => (
                    <div className="flex flex-col gap-1" key={hot.path}>
                        <div className="flex items-baseline justify-between gap-3">
                            <span className="truncate font-mono text-[12px]">{hot.path}</span>
                            <span className="shrink-0 font-mono text-[11.5px] text-dim tabular-nums">
                                {hot.changes} change{hot.changes === 1 ? '' : 's'}
                            </span>
                        </div>
                        <span className="h-1 overflow-hidden rounded-full bg-line">
                            <span
                                className="block h-full rounded-full bg-add-line"
                                style={{ width: `${Math.max(4, (hot.changes / peak) * 100)}%` }}
                            />
                        </span>
                    </div>
                ))}
            </div>
        </section>
    );
}

function Intro({ facts, shown }: { facts: Facts | null; shown: null | Overview }) {
    const branch = useRepo((store) => store.status.branch.head);

    return (
        <header className="flex flex-col gap-3">
            <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
                <h1 className="selectable m-0 text-[30px] leading-tight font-semibold tracking-tight">
                    {facts?.name}
                </h1>
                <span className="font-mono text-[12px] text-dim">
                    {[branch, facts?.headSha?.slice(0, 7)].filter(Boolean).join(' · ')}
                </span>
            </div>
            {shown?.purpose ? (
                <p className="selectable m-0 max-w-[760px] text-[16px] leading-relaxed text-fg">
                    {shown.purpose}
                </p>
            ) : facts?.description ? (
                <blockquote className="selectable m-0 max-w-[760px] border-l-2 border-line pl-3 leading-relaxed text-soft">
                    {facts.description.text}{' '}
                    <span className="font-mono text-[11.5px] text-dim">
                        {facts.description.source}
                    </span>
                </blockquote>
            ) : (
                facts && (
                    <p className="m-0 text-muted">
                        No README or manifest description yet. The overview can write one.
                    </p>
                )
            )}
        </header>
    );
}

/** Before the first overview: what it adds to the facts above, and one button. */
function Invitation({ state }: { state: OverviewState }) {
    const { startOverview } = useRepo();

    if (state.status === 'running') {
        return (
            <div className="flex flex-col gap-3">
                {['Architecture', 'What lives where', 'Where to start'].map((title) => (
                    <section
                        className="flex flex-col gap-2 rounded-xl border border-dashed border-accent-line p-5"
                        key={title}
                    >
                        <span className="text-[15px] font-semibold">{title}</span>
                        <span className="h-2 w-2/3 animate-pulse rounded-full bg-line" />
                        <span className="h-2 w-1/2 animate-pulse rounded-full bg-line" />
                    </section>
                ))}
            </div>
        );
    }

    return (
        <section className="flex flex-wrap items-center justify-between gap-4 rounded-xl border border-dashed border-accent-line bg-accent-bg p-5">
            <div className="flex max-w-[620px] flex-col gap-1.5">
                <h2 className="m-0 text-[16px] font-semibold">
                    Map how this project fits together
                </h2>
                <p className="m-0 leading-relaxed text-body">
                    The agent explores the repository read-only and writes what it does, a diagram
                    of its architecture, what lives where and the files to read first. Seam then
                    tracks which parts the new commits make out of date, so you refresh only those.
                </p>
            </div>
            <div className="flex flex-col items-end gap-1.5">
                <Button onClick={() => startOverview()} size="lg" variant="primary">
                    <Icon name="sparkle" size={13} /> Write the overview
                </Button>
                <span className="text-[12px] text-dim">
                    About 2–5 minutes · runs in the background
                </span>
            </div>
        </section>
    );
}

function OverviewHeader({
    drift,
    shown,
    state,
}: {
    drift: Drift | null;
    shown: null | Overview;
    state: OverviewState;
}) {
    const { startOverview } = useRepo();
    const running = state.status === 'running';
    const drifted = drift?.sections ?? [];

    return (
        <div className="flex shrink-0 flex-wrap items-center gap-2.5 border-b border-line px-[18px] py-2.5">
            {state.status === 'running' ? (
                <RunStatus state={state} />
            ) : (
                <Freshness drift={drift} shown={shown} />
            )}
            <span className="flex-1" />
            <HistoryMenu disabled={running} shownId={shown?.id ?? null} />
            <ProviderSelect disabled={running} />
            <Button
                aria-label="AI settings"
                onClick={() => useSettingsModal.getState().show('ai')}
                size="sm"
                title="AI settings"
                variant="ghost"
            >
                <Icon name="gear" size={13} />
            </Button>
            {!running && shown && drifted.length > 0 && (
                <Button
                    onClick={() => startOverview(drifted)}
                    size="sm"
                    title="Rewrite only the sections the new commits touch"
                    variant="primary"
                >
                    <Icon name="sparkle" size={12} /> Update {drifted.length} section
                    {drifted.length === 1 ? '' : 's'}
                </Button>
            )}
            {!running && shown && (
                <Button
                    onClick={() => startOverview()}
                    size="sm"
                    title="Write the whole overview again"
                >
                    <Icon name="refresh" size={12} /> Rewrite all
                </Button>
            )}
        </div>
    );
}

function Pill({ children, tone }: { children: ReactNode; tone: 'fresh' | 'warn' }) {
    return (
        <span
            className={`flex min-w-0 items-center gap-2 rounded-full px-2.5 py-0.5 text-[12px] ${tone === 'warn' ? 'bg-warn-bg text-warn' : 'bg-control text-soft'}`}
        >
            <span
                className={`h-1.5 w-1.5 shrink-0 rounded-full ${tone === 'warn' ? 'bg-del' : 'bg-add'}`}
            />
            <span className="truncate">{children}</span>
        </span>
    );
}

/** The header's half of a run: who is writing, for how long, and a way to stop it. */
function RunStatus({ state }: { state: RunningState }) {
    const { cancelOverview } = useRepo();
    const clock = useClock(state.startedAt);
    const count = state.sections.length;

    return (
        <>
            <span className="flex min-w-0 items-center gap-2 text-[12.5px] text-soft" role="status">
                <span className="h-1.5 w-1.5 shrink-0 animate-pulse rounded-full bg-accent" />
                <span className="truncate">
                    {PROVIDER_LABELS[state.provider]} is{' '}
                    {count
                        ? `updating ${count} section${count === 1 ? '' : 's'}`
                        : 'writing the overview'}
                </span>
                <span className="font-mono text-[12px] text-muted">{clock}</span>
            </span>
            <span className="hidden text-[12px] text-dim xl:inline">
                Keeps going if you close the window
            </span>
            <Button onClick={cancelOverview} size="sm">
                Cancel
            </Button>
        </>
    );
}

function Scripts({ facts }: { facts: Facts }) {
    const notify = useRepo((store) => store.notify);

    const copy = (text: string) =>
        navigator.clipboard.writeText(text).then(
            () => notify(`Copied ${text}`),
            () => notify('Couldn’t copy to the clipboard'),
        );

    return (
        <section className={CARD}>
            <CardTitle source="manifests">Run it</CardTitle>
            <div className="flex flex-col">
                {facts.scripts.map((script) => (
                    <button
                        className="-mx-2 flex items-baseline justify-between gap-3 rounded-md px-2 py-1 text-left hover:bg-hover"
                        key={script.run}
                        onClick={() => void copy(script.run)}
                        title={
                            script.command ? `${script.command} · click to copy` : 'Click to copy'
                        }
                        type="button"
                    >
                        <span className="shrink-0 font-mono text-[12px]">{script.run}</span>
                        <span className="truncate font-mono text-[11.5px] text-dim">
                            {script.command}
                        </span>
                    </button>
                ))}
            </div>
        </section>
    );
}

function SectionBlock({ block }: { block: VisualBlock }) {
    switch (block.type) {
        case 'code':
            return <CodeCard block={block} />;
        case 'diagram':
            return <Diagram diagram={block} />;
        case 'fileTree':
            return <FileTreeCard block={block} diffs={[]} />;
        case 'text':
            return <Prose markdown={block.markdown} />;
        default:
            return null;
    }
}

function Sections({
    drift,
    overview,
    state,
}: {
    drift: Drift | null;
    overview: Overview;
    state: OverviewState;
}) {
    const { startOverview } = useRepo();
    const running = state.status === 'running';
    const rewriting = (section: OverviewSection) =>
        state.status === 'running' &&
        (!state.sections.length || state.sections.includes(section.id));

    return (
        <div className="flex flex-col gap-8">
            {overview.sections.map((section) => {
                const stale = !!drift?.sections.includes(section.id);
                const edited = !!drift?.minor.includes(section.id);

                return (
                    <section className="flex flex-col gap-3" key={section.id}>
                        <div className="flex flex-wrap items-center gap-2.5">
                            <h2 className="m-0 text-[17px] font-semibold">{section.title}</h2>
                            {rewriting(section) ? (
                                <span className="flex items-center gap-1.5 rounded-full bg-accent-bg px-2 py-0.5 text-[11.5px] text-accent-strong">
                                    <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-accent" />
                                    Rewriting
                                </span>
                            ) : stale ? (
                                <span className="rounded-full bg-warn-bg px-2 py-0.5 text-[11.5px] text-warn">
                                    Changed since it was written
                                </span>
                            ) : (
                                edited && (
                                    <span className="rounded-full bg-control px-2 py-0.5 text-[11.5px] text-muted">
                                        Small edits since
                                    </span>
                                )
                            )}
                            <span className="flex-1" />
                            {(stale || edited) && !running && (
                                <Button
                                    onClick={() => startOverview([section.id])}
                                    size="sm"
                                    title={`Rewrite ${section.title} from the new commits`}
                                    variant={stale ? 'default' : 'ghost'}
                                >
                                    <Icon name="sparkle" size={12} /> Update section
                                </Button>
                            )}
                        </div>
                        {section.paths.length > 0 && (
                            <span className="-mt-1.5 truncate font-mono text-[11.5px] text-dim">
                                Covers {section.paths.join(', ')}
                            </span>
                        )}
                        <div
                            className={`flex flex-col gap-3 ${rewriting(section) ? 'opacity-60' : ''}`}
                        >
                            {section.blocks.map((block, index) => (
                                <SectionBlock block={block} key={index} />
                            ))}
                        </div>
                    </section>
                );
            })}
        </div>
    );
}

function shownOverview(state: OverviewState): null | Overview {
    if (state.status === 'done') {
        return state.overview;
    }

    return state.status === 'idle' ? null : state.previous;
}

function Stack({ facts }: { facts: Facts }) {
    const total = facts.languages.reduce((sum, language) => sum + language.bytes, 0);

    return (
        <section className={CARD}>
            <CardTitle source="manifests">Stack</CardTitle>
            {facts.stack.length ? (
                <div className="flex flex-wrap gap-1.5">
                    {facts.stack.map((tech) => (
                        <span
                            className="inline-flex items-baseline gap-1.5 rounded-md border border-control-line bg-control px-2 py-0.5 text-[12px]"
                            key={tech.name}
                            title={`From ${tech.source}`}
                        >
                            {tech.name}
                            {tech.version && (
                                <span className="font-mono text-[10.5px] text-dim">
                                    {tech.version}
                                </span>
                            )}
                        </span>
                    ))}
                </div>
            ) : (
                <span className="text-[12.5px] text-muted">No known manifests found.</span>
            )}
            {total > 0 && (
                <>
                    <div className="flex h-2 gap-0.5 overflow-hidden rounded-full">
                        {facts.languages.map((language, index) => (
                            <span
                                key={language.name}
                                style={{
                                    background: LANGUAGE_COLORS[index % LANGUAGE_COLORS.length],
                                    flexGrow: language.bytes,
                                }}
                                title={language.name}
                            />
                        ))}
                    </div>
                    <div className="flex flex-wrap gap-x-3.5 gap-y-1 font-mono text-[11.5px] text-muted">
                        {facts.languages.map((language, index) => (
                            <span className="flex items-center gap-1.5" key={language.name}>
                                <span
                                    className="h-2 w-2 rounded-sm"
                                    style={{
                                        background: LANGUAGE_COLORS[index % LANGUAGE_COLORS.length],
                                    }}
                                />
                                {language.name}{' '}
                                {Math.max(1, Math.round((language.bytes / total) * 100))}%
                            </span>
                        ))}
                    </div>
                </>
            )}
        </section>
    );
}

function Stat({ label, value }: { label: string; value: string }) {
    return (
        <div className="flex min-w-0 flex-col">
            <span className="truncate font-mono text-[20px] text-fg tabular-nums">{value}</span>
            <span className="text-[12px] text-muted">{label}</span>
        </div>
    );
}
