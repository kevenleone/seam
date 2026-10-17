import { useMemo, useState } from 'react';

import type { DiffLine, FileDiff, Finding, VisualAnnotation, VisualDiff } from '../../lib/types';

import { DiffView } from '../DiffView';
import { Note } from './Blocks';
import { Inline } from './Markdown';
import { useVisual } from './mode';

const STATIC_LINE_LIMIT = 400;

/** The key files' real diffs, as tabs in the app and one after another on paper. */
export function KeyChanges({ blocks, diffs }: { blocks: VisualDiff[]; diffs: FileDiff[] }) {
    const { mode } = useVisual();
    const shown = blocks
        .map((block) => ({ block, diff: diffs.find((diff) => pathOf(diff) === block.file) }))
        .filter((entry): entry is { block: VisualDiff; diff: FileDiff } => !!entry.diff);

    if (!shown.length) {
        return null;
    }

    if (mode !== 'app') {
        return (
            <div className="flex flex-col gap-3">
                {shown.map(({ block, diff }) => (
                    <section
                        className="overflow-hidden rounded-xl border border-line bg-panel"
                        key={block.file}
                    >
                        <DiffHeader block={block} diff={diff} />
                        <StaticDiff annotations={block.annotations} diff={diff} />
                    </section>
                ))}
            </div>
        );
    }

    return <DiffTabs shown={shown} />;
}

function DiffHeader({ block, diff }: { block: VisualDiff; diff: FileDiff }) {
    return (
        <header className="flex flex-col gap-0.5 border-b border-line px-4 py-2.5">
            <span className="flex items-center gap-2 font-mono text-[12.5px]">
                {block.file}
                <span className="text-[11.5px]">
                    <span className="text-add">+{diff.additions}</span>{' '}
                    <span className="text-del">−{diff.deletions}</span>
                </span>
            </span>
            {block.summary && <Inline className="text-[12.5px] text-muted" text={block.summary} />}
        </header>
    );
}

const PANEL_LIMIT = 520;

function DiffTabs({ shown }: { shown: { block: VisualDiff; diff: FileDiff }[] }) {
    const [active, setActive] = useState(0);
    const [dismissed, setDismissed] = useState<Record<string, number[]>>({});
    const { block, diff } = shown[Math.min(active, shown.length - 1)];
    const notes = useMemo(
        () =>
            findings(block).filter(
                (finding) => !(dismissed[block.file] ?? []).includes(finding.id),
            ),
        [block, dismissed],
    );

    return (
        <section className="overflow-hidden rounded-xl border border-line bg-panel">
            <div
                className="flex gap-0.5 overflow-x-auto border-b border-line px-2 pt-2"
                role="tablist"
            >
                {shown.map((entry, index) => {
                    const name = entry.block.file.split('/').pop();

                    return (
                        <button
                            aria-selected={index === active}
                            className={`shrink-0 rounded-t-md border border-b-0 px-3 py-1.5 font-mono text-[12px] ${index === active ? 'border-line bg-app text-fg' : 'border-transparent text-muted hover:text-fg'}`}
                            key={entry.block.file}
                            onClick={() => setActive(index)}
                            role="tab"
                            title={entry.block.file}
                            type="button"
                        >
                            {name}
                        </button>
                    );
                })}
            </div>
            {block.summary && (
                <div className="border-b border-line px-4 py-2">
                    <Inline className="text-[12.5px] text-muted" text={block.summary} />
                </div>
            )}
            <div
                className="flex flex-col bg-app"
                style={{ height: panelHeight(diff, notes.length) }}
            >
                <DiffView
                    comments={{
                        findings: notes,
                        focus: null,
                        onDismiss: (id) =>
                            setDismissed({
                                ...dismissed,
                                [block.file]: [...(dismissed[block.file] ?? []), id],
                            }),
                    }}
                    diffs={[diff]}
                    key={block.file}
                />
            </div>
        </section>
    );
}

function findings(block: VisualDiff): Finding[] {
    return block.annotations.map((annotation, id) => ({
        body: '',
        file: block.file,
        id,
        line: annotation.side === 'before' ? null : annotation.line,
        severity: 'note',
        suggestion: null,
        title: annotation.note,
    }));
}

/** Tall enough for a short diff and its notes, scrolling past `PANEL_LIMIT`. */
function panelHeight(diff: FileDiff, notes: number): number {
    const lines = diff.hunks.reduce((sum, hunk) => sum + hunk.lines.length + 1, 0);

    return Math.min(PANEL_LIMIT, 48 + lines * 22 + notes * 110);
}

function pathOf(diff: FileDiff): string {
    return diff.newPath ?? diff.oldPath ?? '';
}

const LINE_TONES: Record<DiffLine['kind'], string> = {
    addition: 'bg-add-bg',
    context: '',
    deletion: 'bg-del-bg',
    noNewline: 'text-dim',
};

const SIGNS: Record<DiffLine['kind'], string> = {
    addition: '+',
    context: ' ',
    deletion: '−',
    noNewline: ' ',
};

/** Every line, unvirtualised, for a print or an exported page. */
function StaticDiff({ annotations, diff }: { annotations: VisualAnnotation[]; diff: FileDiff }) {
    let budget = STATIC_LINE_LIMIT;
    const total = diff.hunks.reduce((sum, hunk) => sum + hunk.lines.length, 0);

    const notesAt = (line: DiffLine) =>
        annotations.filter((annotation) =>
            annotation.side === 'before'
                ? line.kind !== 'addition' && annotation.line === line.oldNumber
                : line.kind !== 'deletion' && annotation.line === line.newNumber,
        );

    return (
        <div className="overflow-x-auto bg-app font-mono text-[11.5px] leading-[1.6]">
            {diff.hunks.map((hunk, hunkIndex) => {
                if (budget <= 0) {
                    return null;
                }

                const lines = hunk.lines.slice(0, budget);

                budget -= lines.length;

                return (
                    <div key={hunkIndex}>
                        <div className="bg-hunk px-3 py-0.5 text-muted">
                            @@ -{hunk.oldStart},{hunk.oldCount} +{hunk.newStart},{hunk.newCount} @@{' '}
                            {hunk.section}
                        </div>
                        {lines.map((line, index) => (
                            <div key={index}>
                                <div className={`flex ${LINE_TONES[line.kind]}`}>
                                    <span className="w-10 shrink-0 pr-2 text-right text-dim">
                                        {line.oldNumber ?? ''}
                                    </span>
                                    <span className="w-10 shrink-0 pr-2 text-right text-dim">
                                        {line.newNumber ?? ''}
                                    </span>
                                    <span
                                        className={`w-4 shrink-0 ${line.kind === 'addition' ? 'text-add' : line.kind === 'deletion' ? 'text-del' : 'text-dim'}`}
                                    >
                                        {SIGNS[line.kind]}
                                    </span>
                                    <span className="pr-4 whitespace-pre-wrap text-body">
                                        {line.text}
                                    </span>
                                </div>
                                {notesAt(line).map((annotation, position) => (
                                    <Note key={position} text={annotation.note} />
                                ))}
                            </div>
                        ))}
                    </div>
                );
            })}
            {total > STATIC_LINE_LIMIT && (
                <div className="px-3 py-2 font-sans text-muted">
                    {total - STATIC_LINE_LIMIT} more lines in this file.
                </div>
            )}
        </div>
    );
}
