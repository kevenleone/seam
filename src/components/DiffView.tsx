import { useVirtualizer } from '@tanstack/react-virtual';
import { useMemo, useRef, useState } from 'react';

import type { DiffLine, FileDiff, Hunk, LineAction } from '../lib/types';

import { useSettings } from '../lib/settings';
import { isChange } from '../lib/store';
import { Icon } from './Icon';
import { Button } from './ui';

/** Line staging controls. Without it the diff is read-only. */
export interface DiffInteraction {
    actions: { action: LineAction; title: string; variant: 'add' | 'default' | 'del' }[];
    apply(action: LineAction, hunks?: number[]): void;
    clear(hunk: number): void;
    selection: Record<number, number[]>;
    toggle(hunk: number, line: number, range: boolean): void;
}

type Row =
    | { collapsed: boolean; diff: FileDiff; t: 'file' }
    | { hunk: Hunk; left: DiffLine | null; right: DiffLine | null; t: 'split' }
    | { hunk: Hunk; line: DiffLine; t: 'line' }
    | { hunk: Hunk; t: 'hunk' }
    | { t: 'note'; text: string };

export function DiffView({
    diffs,
    interaction,
}: {
    diffs: FileDiff[];
    interaction?: DiffInteraction;
}) {
    const { codeSize, splitDiff } = useSettings();
    const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
    const multi = diffs.length > 1 || !interaction;
    const lineHeight = Math.round(codeSize * 1.72);

    const { maxChars, rows } = useMemo(() => {
        const rows: Row[] = [];
        let maxChars = 0;

        for (const diff of diffs) {
            const id = diff.newPath ?? diff.oldPath ?? '';
            const isCollapsed = collapsed.has(id);

            if (multi) {
                rows.push({ collapsed: isCollapsed, diff, t: 'file' });
            }

            if (isCollapsed) {
                continue;
            }

            if (diff.isBinary) {
                rows.push({ t: 'note', text: 'Binary file changed' });
            } else if (!diff.hunks.length) {
                rows.push({
                    t: 'note',
                    text: diff.headerLines.some((headerLine) => headerLine.startsWith('old mode'))
                        ? 'File mode changed'
                        : 'No content changes',
                });
            }

            for (const hunk of diff.hunks) {
                rows.push({ hunk, t: 'hunk' });

                if (splitDiff) {
                    rows.push(...splitRows(hunk));
                } else {
                    for (const line of hunk.lines) {
                        rows.push({ hunk, line, t: 'line' });
                    }
                }

                for (const line of hunk.lines) {
                    maxChars = Math.max(maxChars, line.text.length);
                }
            }
        }

        return { maxChars, rows };
    }, [diffs, splitDiff, collapsed, multi]);

    const scrollRef = useRef<HTMLDivElement>(null);
    const virtualizer = useVirtualizer({
        count: rows.length,
        estimateSize: (index) =>
            rows[index].t === 'file'
                ? 36
                : rows[index].t === 'hunk'
                  ? 32
                  : rows[index].t === 'note'
                    ? 64
                    : lineHeight,
        getScrollElement: () => scrollRef.current,
        overscan: 30,
    });

    const toggleFile = (id: string) =>
        setCollapsed((prev) => {
            const next = new Set(prev);

            if (next.has(id)) {
                next.delete(id);
            } else {
                next.add(id);
            }

            return next;
        });

    // Rows are absolutely positioned, so give the content an explicit width for horizontal scrolling.
    const contentWidth = splitDiff ? '100%' : `max(100%, calc(${maxChars + 4}ch + 110px))`;

    return (
        <div
            className="relative min-h-0 flex-1 overflow-auto"
            ref={scrollRef}
            style={{ fontSize: codeSize }}
        >
            <div
                className="font-mono"
                style={{
                    height: virtualizer.getTotalSize(),
                    position: 'relative',
                    width: contentWidth,
                }}
            >
                {virtualizer.getVirtualItems().map((item) => {
                    const row = rows[item.index];

                    return (
                        <div
                            data-index={item.index}
                            key={item.key}
                            ref={virtualizer.measureElement}
                            style={{
                                left: 0,
                                position: 'absolute',
                                top: item.start,
                                width: '100%',
                            }}
                        >
                            {row.t === 'file' && (
                                <FileHeader
                                    collapsed={row.collapsed}
                                    diff={row.diff}
                                    onToggle={() =>
                                        toggleFile(row.diff.newPath ?? row.diff.oldPath ?? '')
                                    }
                                />
                            )}
                            {row.t === 'hunk' && (
                                <HunkHeader hunk={row.hunk} interaction={interaction} />
                            )}
                            {row.t === 'note' && (
                                <div className="flex h-16 items-center justify-center font-sans text-muted">
                                    {row.text}
                                </div>
                            )}
                            {row.t === 'line' && (
                                <UnifiedLine
                                    height={lineHeight}
                                    hunk={row.hunk}
                                    interaction={interaction}
                                    line={row.line}
                                />
                            )}
                            {row.t === 'split' && (
                                <SplitLine
                                    height={lineHeight}
                                    interaction={interaction}
                                    row={row}
                                />
                            )}
                        </div>
                    );
                })}
            </div>
        </div>
    );
}

function FileHeader({
    collapsed,
    diff,
    onToggle,
}: {
    collapsed: boolean;
    diff: FileDiff;
    onToggle: () => void;
}) {
    const path = diff.newPath ?? diff.oldPath ?? '';
    const slash = path.lastIndexOf('/');

    return (
        <button
            className="flex h-9 w-full items-center gap-2 border-y border-line bg-panel px-3 text-left font-sans"
            onClick={onToggle}
            type="button"
        >
            <Icon
                className="text-muted"
                name={collapsed ? 'chevronRight' : 'chevronDown'}
                size={12}
            />
            <span className="font-mono text-[12.5px]">
                <span className="text-muted">{path.slice(0, slash + 1)}</span>
                {path.slice(slash + 1)}
            </span>
            <span className="text-[12px] text-add">+{diff.additions}</span>
            <span className="text-[12px] text-del">−{diff.deletions}</span>
        </button>
    );
}

function HunkHeader({ hunk, interaction }: { hunk: Hunk; interaction?: DiffInteraction }) {
    const selected = interaction?.selection[hunk.id]?.length ?? 0;

    return (
        <div className="sticky left-0 flex h-8 items-center gap-2 bg-hunk px-3 font-sans">
            <span className="truncate font-mono text-[0.92em] text-muted">
                @@ -{hunk.oldStart},{hunk.oldCount} +{hunk.newStart},{hunk.newCount} @@{' '}
                {hunk.section}
            </span>
            <span className="flex-1" />
            {interaction &&
                (selected > 0 ? (
                    <>
                        <span className="text-[12px] text-accent">
                            {selected} line{selected === 1 ? '' : 's'} selected
                        </span>
                        {interaction.actions.map((action) => (
                            <Button
                                key={action.action}
                                onClick={() => interaction.apply(action.action)}
                                size="sm"
                                variant={action.variant}
                            >
                                {action.title} lines
                            </Button>
                        ))}
                        <Button
                            onClick={() => interaction.clear(hunk.id)}
                            size="sm"
                            variant="ghost"
                        >
                            Clear
                        </Button>
                    </>
                ) : (
                    interaction.actions.map((action) => (
                        <Button
                            key={action.action}
                            onClick={() => interaction.apply(action.action, [hunk.id])}
                            size="sm"
                            variant={action.variant}
                        >
                            {action.title} hunk
                        </Button>
                    ))
                ))}
        </div>
    );
}

/** Pairs each run of deletions with the additions that follow it. */
function splitRows(hunk: Hunk): Row[] {
    const rows: Row[] = [];
    let dels: DiffLine[] = [];
    let adds: DiffLine[] = [];

    const flush = () => {
        for (let i = 0; i < Math.max(dels.length, adds.length); i++) {
            rows.push({ hunk, left: dels[i] ?? null, right: adds[i] ?? null, t: 'split' });
        }

        dels = [];
        adds = [];
    };

    for (const line of hunk.lines) {
        if (line.kind === 'deletion') {
            if (adds.length) {
                flush();
            }

            dels.push(line);
        } else if (line.kind === 'addition') {
            adds.push(line);
        } else {
            flush();
            rows.push({ hunk, left: line, right: line, t: 'split' });
        }
    }

    flush();

    return rows;
}

const styles = {
    addition: {
        bg: 'bg-add-bg',
        gutter: 'bg-add-gutter',
        num: 'text-add-num',
        sign: '+',
        signColor: 'text-add',
    },
    context: { bg: '', gutter: '', num: 'text-dim', sign: '', signColor: 'text-dim' },
    deletion: {
        bg: 'bg-del-bg',
        gutter: 'bg-del-gutter',
        num: 'text-del-num',
        sign: '−',
        signColor: 'text-del',
    },
    noNewline: { bg: '', gutter: '', num: 'text-dim', sign: '', signColor: 'text-dim' },
};

function Code({ height, line }: { height: number; line: DiffLine }) {
    if (line.kind === 'noNewline') {
        return (
            <span
                className="pl-1.5 font-sans text-[12px] text-muted italic"
                style={{ lineHeight: `${height}px` }}
            >
                No newline at end of file
            </span>
        );
    }

    return (
        <span
            className="selectable pr-4 pl-1.5 whitespace-pre text-body"
            style={{ lineHeight: `${height}px`, tabSize: 4 }}
        >
            {line.text}
        </span>
    );
}

function SplitLine({
    height,
    interaction,
    row,
}: {
    height: number;
    interaction?: DiffInteraction;
    row: Extract<Row, { t: 'split' }>;
}) {
    return (
        <div className="grid grid-cols-2" style={{ height }}>
            <SplitSide
                height={height}
                hunk={row.hunk}
                interaction={interaction}
                line={row.left}
                number={row.left?.oldNumber ?? null}
            />
            <SplitSide
                border
                height={height}
                hunk={row.hunk}
                interaction={interaction}
                line={row.right}
                number={row.right?.newNumber ?? null}
            />
        </div>
    );
}

function SplitSide({
    border,
    height,
    hunk,
    interaction,
    line,
    number,
}: {
    border?: boolean;
    height: number;
    hunk: Hunk;
    interaction?: DiffInteraction;
    line: DiffLine | null;
    number: null | number;
}) {
    const { onGutter, selectable, selected } = useLineSelection(hunk, line, interaction);

    if (!line) {
        return (
            <div
                className={`bg-[repeating-linear-gradient(135deg,transparent_0_6px,var(--line)_6px_7px)] ${border ? 'border-l border-line' : ''}`}
            />
        );
    }

    const s = styles[line.kind];

    return (
        <div
            className={`flex min-w-0 overflow-hidden ${s.bg} ${border ? 'border-l border-line' : ''}`}
        >
            <span
                aria-pressed={selectable ? selected : undefined}
                className={`w-12 shrink-0 pr-2 text-right ${s.num} ${selected ? 'bg-accent-line' : s.gutter} ${selectable ? 'cursor-pointer' : ''}`}
                onClick={onGutter}
                role={selectable ? 'button' : undefined}
                style={{
                    boxShadow: selected ? 'inset 3px 0 0 var(--accent)' : undefined,
                    lineHeight: `${height}px`,
                }}
            >
                {number ?? ''}
            </span>
            <span
                className={`w-4 shrink-0 text-center ${s.signColor}`}
                style={{ lineHeight: `${height}px` }}
            >
                {s.sign}
            </span>
            <Code height={height} line={line} />
        </div>
    );
}

function UnifiedLine({
    height,
    hunk,
    interaction,
    line,
}: {
    height: number;
    hunk: Hunk;
    interaction?: DiffInteraction;
    line: DiffLine;
}) {
    const s = styles[line.kind];
    const { onGutter, selectable, selected } = useLineSelection(hunk, line, interaction);

    return (
        <div className={`flex ${s.bg}`} style={{ height }}>
            <div
                aria-label={
                    selectable
                        ? `${selected ? 'Deselect' : 'Select'} line ${line.newNumber ?? line.oldNumber}`
                        : undefined
                }
                aria-pressed={selectable ? selected : undefined}
                className={`sticky left-0 flex shrink-0 ${selected ? 'bg-accent-line' : s.gutter || 'bg-app'} ${selectable ? 'cursor-pointer' : ''}`}
                onClick={onGutter}
                onKeyDown={(event) =>
                    selectable &&
                    (event.key === ' ' || event.key === 'Enter') &&
                    interaction!.toggle(hunk.id, line.id, event.shiftKey)
                }
                role={selectable ? 'button' : undefined}
                style={{ boxShadow: selected ? 'inset 3px 0 0 var(--accent)' : undefined }}
                tabIndex={selectable ? 0 : undefined}
                title={selectable ? 'Click to select, shift-click for a range' : undefined}
            >
                <span
                    className={`w-11 pr-2 text-right ${s.num}`}
                    style={{ lineHeight: `${height}px` }}
                >
                    {line.oldNumber ?? ''}
                </span>
                <span
                    className={`w-11 pr-2 text-right ${s.num}`}
                    style={{ lineHeight: `${height}px` }}
                >
                    {line.newNumber ?? ''}
                </span>
                <span
                    className={`w-5 text-center ${s.signColor}`}
                    style={{ lineHeight: `${height}px` }}
                >
                    {s.sign}
                </span>
            </div>
            <Code height={height} line={line} />
        </div>
    );
}

function useLineSelection(hunk: Hunk, line: DiffLine | null, interaction?: DiffInteraction) {
    const selectable = !!interaction && !!line && isChange(line.kind);
    const selected = selectable && (interaction!.selection[hunk.id]?.includes(line!.id) ?? false);

    const onGutter = (event: React.MouseEvent) => {
        if (selectable) {
            interaction!.toggle(hunk.id, line!.id, event.shiftKey);
        }
    };

    return { onGutter, selectable, selected };
}
