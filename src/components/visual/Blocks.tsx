import { useMemo, useState } from 'react';

import type {
    FileDiff,
    VisualAnnotation,
    VisualCanvas,
    VisualChange,
    VisualCode,
    VisualCompare,
    VisualContract,
    VisualDecision,
    VisualFileTree,
    VisualKind,
    VisualQuestions,
    VisualSide,
    VisualVerify,
} from '../../lib/types';

import { CHANGE_LABELS, surfaceOf } from '../../lib/visual';
import { ScreenFrame, StateChip } from './Canvas';
import { DiffPanel } from './DiffPanel';
import { Inline, Prose } from './Markdown';
import { useVisual } from './mode';

const CARD = 'rounded-xl border border-line bg-panel';

const CHANGE_TONES: Record<Exclude<VisualChange, ''>, string> = {
    added: 'bg-add-bg text-add',
    modified: 'bg-accent-bg text-accent-strong',
    removed: 'bg-del-bg text-del',
    renamed: 'bg-control text-muted',
};

export function ChangeChip({ change }: { change: VisualChange }) {
    if (!change) {
        return null;
    }

    return (
        <span
            className={`shrink-0 rounded-full px-1.5 py-px text-[10.5px] font-semibold tracking-wide uppercase ${CHANGE_TONES[change]}`}
        >
            {CHANGE_LABELS[change]}
        </span>
    );
}

export function CodeCard({ block }: { block: VisualCode }) {
    return (
        <section className={`${CARD} overflow-hidden`}>
            <header className="flex flex-col gap-0.5 border-b border-line px-4 py-2.5">
                <span className="font-mono text-[12.5px]">{block.file || block.language}</span>
                {block.summary && (
                    <Inline className="text-[12.5px] text-muted" text={block.summary} />
                )}
            </header>
            <CodeLines
                annotations={block.annotations}
                code={block.code}
                file={block.file}
                language={block.language}
            />
        </section>
    );
}

export function CodeLines({
    annotations = [],
    code,
    file = '',
    language = '',
}: {
    annotations?: VisualAnnotation[];
    code: string;
    file?: string;
    language?: string;
}) {
    const lines = useCodeLines(code, language, file);

    return (
        <div className="selectable overflow-x-auto bg-app py-2 font-mono text-[12px] leading-[1.7]">
            {lines.map((html, index) => (
                <div key={index}>
                    <div className="flex">
                        <span className="w-10 shrink-0 pr-3 text-right text-dim select-none">
                            {index + 1}
                        </span>
                        <span
                            className="pr-4 whitespace-pre text-body"
                            dangerouslySetInnerHTML={{ __html: html || ' ' }}
                        />
                    </div>
                    {annotations
                        .filter((annotation) => annotation.line === index + 1)
                        .map((annotation, position) => (
                            <Note key={position} text={annotation.note} />
                        ))}
                </div>
            ))}
        </div>
    );
}

export function CompareCard({
    block,
    canvas,
    kind,
}: {
    block: VisualCompare;
    canvas: VisualCanvas;
    kind: VisualKind;
}) {
    return (
        <section className="flex flex-col gap-2">
            {block.title && (
                <h3 className="m-0 text-[14px] font-semibold">
                    <Inline text={block.title} />
                </h3>
            )}
            <div className="grid grid-cols-2 gap-3">
                <CompareSide canvas={canvas} label="before" side={block.before} />
                <CompareSide
                    canvas={canvas}
                    label={kind === 'plan' ? 'proposed' : 'after'}
                    side={block.after}
                />
            </div>
        </section>
    );
}

export function Heading({ children }: { children: string }) {
    return <h2 className="m-0 mt-2 text-[17px] font-semibold">{children}</h2>;
}

export function Note({ text }: { text: string }) {
    return (
        <div className="sticky left-0 my-1 ml-10 max-w-[640px] rounded-md border border-accent-line bg-accent-bg px-2.5 py-1.5 font-sans text-[12.5px] leading-normal text-body">
            <Inline text={text} />
        </div>
    );
}

function CompareSide({
    canvas,
    label,
    side,
}: {
    canvas: VisualCanvas;
    label: 'after' | 'before' | 'proposed';
    side: VisualSide;
}) {
    const screen = side.screen ? canvas.screens.find((each) => each.id === side.screen) : null;

    return (
        <div className={`${CARD} flex min-w-0 flex-col overflow-hidden`}>
            <div className="border-b border-line px-3 py-2">
                <StateChip state={label} />
            </div>
            {screen ? (
                <ScaledScreen canvas={canvas} screen={screen} />
            ) : side.code ? (
                <CodeLines code={side.code} language={side.language ?? ''} />
            ) : (
                <div className="p-3">
                    <Prose markdown={side.text ?? ''} />
                </div>
            )}
        </div>
    );
}

/** Highlighted lines when the language is known, plain text otherwise. */
function useCodeLines(code: string, language: string, file: string): string[] {
    const { highlighter } = useVisual();

    return useMemo(() => {
        const lines = code.replace(/\n$/, '').split('\n');
        const known =
            highlighter &&
            (highlighter.languageFor(file) ?? highlighter.languageFor(`x.${language}`));

        if (!highlighter || !known) {
            return lines.map((line) =>
                line.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;'),
            );
        }

        return highlighter.highlightLines(lines, known);
    }, [code, file, highlighter, language]);
}

const COMPARE_WIDTH = 380;

export function ContractCard({ block }: { block: VisualContract }) {
    return (
        <section className="flex flex-col gap-2">
            {block.title && (
                <h3 className="m-0 text-[14px] font-semibold">
                    <Inline text={block.title} />
                </h3>
            )}
            {block.endpoints.length > 0 && (
                <div className={`${CARD} flex flex-col divide-y divide-line`}>
                    {block.endpoints.map((endpoint, index) => (
                        <details className="group px-4 py-2" key={index} open={false}>
                            <summary className="flex cursor-pointer list-none items-center gap-2.5">
                                <span className="text-dim transition-transform group-open:rotate-90">
                                    ›
                                </span>
                                <span className="rounded bg-accent-bg px-1.5 font-mono text-[11px] font-semibold text-accent-strong">
                                    {endpoint.method || '—'}
                                </span>
                                <span className="font-mono text-[12.5px]">{endpoint.path}</span>
                                <ChangeChip change={endpoint.change} />
                                <Inline
                                    className="min-w-0 flex-1 truncate text-[12.5px] text-muted"
                                    text={endpoint.summary}
                                />
                            </summary>
                            {(endpoint.request || endpoint.response) && (
                                <div className="mt-2 grid grid-cols-2 gap-2">
                                    {(['request', 'response'] as const).map(
                                        (part) =>
                                            endpoint[part] && (
                                                <div
                                                    className="flex min-w-0 flex-col gap-1"
                                                    key={part}
                                                >
                                                    <span className="text-[11px] font-semibold tracking-[0.08em] text-muted uppercase">
                                                        {part}
                                                    </span>
                                                    <pre className="selectable m-0 overflow-x-auto rounded-md border border-line bg-app p-2 font-mono text-[11.5px] text-body">
                                                        {endpoint[part]}
                                                    </pre>
                                                </div>
                                            ),
                                    )}
                                </div>
                            )}
                        </details>
                    ))}
                </div>
            )}
            {block.entities.map((entity) => (
                <div className={`${CARD} overflow-hidden`} key={entity.name}>
                    <header className="flex items-center gap-2 border-b border-line px-4 py-2">
                        <span className="font-mono text-[12.5px] font-semibold">{entity.name}</span>
                        <ChangeChip change={entity.change} />
                    </header>
                    <table className="w-full border-collapse text-[12.5px]">
                        <tbody>
                            {entity.fields.map((field) => (
                                <tr className="border-b border-line last:border-0" key={field.name}>
                                    <td className="w-1/3 px-4 py-1.5 font-mono">{field.name}</td>
                                    <td className="px-2 py-1.5 font-mono text-muted">
                                        {field.was && (
                                            <span className="mr-1.5 text-del line-through">
                                                {field.was}
                                            </span>
                                        )}
                                        {field.type}
                                    </td>
                                    <td className="px-4 py-1.5 text-right">
                                        <ChangeChip change={field.change} />
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </div>
            ))}
        </section>
    );
}

export function DecisionCard({ block }: { block: VisualDecision }) {
    return (
        <section className={`${CARD} flex flex-col gap-3 p-4`}>
            <header className="flex items-center gap-2">
                <span className="rounded-full bg-accent-bg px-2 py-0.5 text-[10.5px] font-semibold tracking-wide text-accent-strong uppercase">
                    Decision
                </span>
                <h3 className="m-0 text-[14px] font-semibold">
                    <Inline text={block.title} />
                </h3>
            </header>
            {block.context && <Labelled label="Context" text={block.context} />}
            {block.decision && <Labelled label="Decision" text={block.decision} />}
            {block.alternatives.length > 0 && (
                <div className="flex flex-col gap-1">
                    <span className="text-[11px] font-semibold tracking-[0.08em] text-muted uppercase">
                        Alternatives considered
                    </span>
                    <ul className="m-0 flex list-none flex-col gap-1.5 p-0">
                        {block.alternatives.map((alternative, index) => (
                            <li className="flex gap-2 leading-normal" key={index}>
                                <span className="text-dim">✕</span>
                                <span>
                                    <Inline className="font-semibold" text={alternative.option} />
                                    {alternative.whyNot && (
                                        <>
                                            <span className="text-muted"> — </span>
                                            <Inline
                                                className="text-body"
                                                text={alternative.whyNot}
                                            />
                                        </>
                                    )}
                                </span>
                            </li>
                        ))}
                    </ul>
                </div>
            )}
            {block.consequences && <Labelled label="Consequences" text={block.consequences} />}
        </section>
    );
}

export function FileTreeCard({ block, diffs }: { block: VisualFileTree; diffs: FileDiff[] }) {
    const { mode } = useVisual();
    const [open, setOpen] = useState<string[]>([]);
    const files = [...block.files].sort((first, second) => first.path.localeCompare(second.path));

    return (
        <section className={`${CARD} flex flex-col overflow-hidden py-1.5`}>
            {files.map((file) => {
                const slash = file.path.lastIndexOf('/');
                const diff =
                    mode === 'app'
                        ? diffs.find((each) => (each.newPath ?? each.oldPath) === file.path)
                        : undefined;
                const shown = !!diff && open.includes(file.path);

                return (
                    <div className="flex flex-col" key={file.path}>
                        <div className="flex flex-col gap-0.5 px-4 py-1.5">
                            <div className="flex items-center gap-2">
                                <ChangeChip change={file.change} />
                                <span className="min-w-0 flex-1 truncate font-mono text-[12.5px]">
                                    <span className="text-muted">
                                        {file.path.slice(0, slash + 1)}
                                    </span>
                                    {file.path.slice(slash + 1)}
                                </span>
                                {(file.additions > 0 || file.deletions > 0) && (
                                    <span className="shrink-0 font-mono text-[11.5px]">
                                        <span className="text-add">+{file.additions}</span>{' '}
                                        <span className="text-del">−{file.deletions}</span>
                                    </span>
                                )}
                                {diff && (
                                    <button
                                        aria-expanded={shown}
                                        className="shrink-0 rounded-md px-2 py-0.5 text-[12px] text-muted hover:bg-hover hover:text-fg"
                                        onClick={() =>
                                            setOpen(
                                                shown
                                                    ? open.filter((path) => path !== file.path)
                                                    : [...open, file.path],
                                            )
                                        }
                                        title={`${shown ? 'Hide' : 'Show'} what changed in ${file.path}`}
                                        type="button"
                                    >
                                        {shown ? 'Hide diff' : 'Diff'}
                                    </button>
                                )}
                            </div>
                            {file.note && (
                                <Inline
                                    className="pl-0.5 text-[12.5px] text-muted"
                                    text={file.note}
                                />
                            )}
                        </div>
                        {diff && shown && (
                            <div className="my-1.5 border-y border-line">
                                <DiffPanel diff={diff} />
                            </div>
                        )}
                    </div>
                );
            })}
        </section>
    );
}

export function QuestionsCard({ block }: { block: VisualQuestions }) {
    return (
        <section className={`${CARD} flex flex-col divide-y divide-line`}>
            {block.items.map((item, index) => (
                <div className="flex gap-3 px-4 py-3" key={index}>
                    <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-accent-bg text-[11px] font-semibold text-accent-strong">
                        {index + 1}
                    </span>
                    <span className="flex flex-col gap-1">
                        <Inline className="font-semibold" text={item.question} />
                        {item.recommendation && (
                            <span className="text-[12.5px] text-muted">
                                Recommended:{' '}
                                <Inline className="text-body" text={item.recommendation} />
                            </span>
                        )}
                    </span>
                </div>
            ))}
        </section>
    );
}

export function VerifyCard({ block }: { block: VisualVerify }) {
    const { mode } = useVisual();
    const [done, setDone] = useState<number[]>([]);

    return (
        <section className={`${CARD} flex flex-col divide-y divide-line`}>
            {block.steps.map((step, index) => {
                const checked = done.includes(index);

                return (
                    <label className="flex cursor-pointer gap-3 px-4 py-2.5" key={index}>
                        {mode === 'app' ? (
                            <input
                                checked={checked}
                                className="mt-1 accent-(--accent)"
                                onChange={() =>
                                    setDone(
                                        checked
                                            ? done.filter((each) => each !== index)
                                            : [...done, index],
                                    )
                                }
                                type="checkbox"
                            />
                        ) : (
                            <span className="mt-1 inline-block h-3.5 w-3.5 shrink-0 rounded-[3px] border border-control-line" />
                        )}
                        <span className={`flex flex-col gap-0.5 ${checked ? 'opacity-55' : ''}`}>
                            <Inline className="font-semibold" text={step.title} />
                            {step.how && <Inline className="text-body" text={step.how} />}
                            {step.expect && (
                                <span className="text-[12.5px] text-muted">
                                    Expect: <Inline text={step.expect} />
                                </span>
                            )}
                        </span>
                    </label>
                );
            })}
        </section>
    );
}

function Labelled({ label, text }: { label: string; text: string }) {
    return (
        <div className="flex flex-col gap-1">
            <span className="text-[11px] font-semibold tracking-[0.08em] text-muted uppercase">
                {label}
            </span>
            <Prose markdown={text} />
        </div>
    );
}

function ScaledScreen({
    canvas,
    screen,
}: {
    canvas: VisualCanvas;
    screen: VisualCanvas['screens'][number];
}) {
    const surface = surfaceOf(screen);
    const scale = Math.min(1, COMPARE_WIDTH / surface.width);

    return (
        <div className="overflow-hidden p-3" style={{ height: surface.height * scale + 24 }}>
            <div
                className="origin-top-left"
                style={{
                    height: surface.height,
                    transform: `scale(${scale})`,
                    width: surface.width,
                }}
            >
                <ScreenFrame canvas={canvas} screen={screen} />
            </div>
        </div>
    );
}
