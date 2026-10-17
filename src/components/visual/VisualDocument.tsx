import type { MouseEvent } from 'react';

import { openUrl } from '@tauri-apps/plugin-opener';

import type { VisualBlock, Visual as VisualDoc } from '../../lib/types';

import { PROVIDER_LABELS } from '../../lib/ai';
import { useSettings } from '../../lib/settings';
import { type Highlighter, useHighlighter } from '../../lib/useHighlighter';
import { installSketchStyle, sections } from '../../lib/visual';
import {
    CodeCard,
    CompareCard,
    ContractCard,
    DecisionCard,
    FileTreeCard,
    Heading,
    QuestionsCard,
    VerifyCard,
} from './Blocks';
import { Canvas } from './Canvas';
import { Diagram } from './Diagram';
import { KeyChanges } from './KeyChanges';
import { Inline, Prose } from './Markdown';
import { type RenderMode, Visual } from './mode';

installSketchStyle();

const HEADINGS: Partial<Record<VisualBlock['type'], string>> = {
    contract: 'Contracts',
    decision: 'Decisions',
    diagram: 'Architecture',
    fileTree: 'Files',
    questions: 'Open questions',
    verify: 'How to test it',
};

export function VisualDocument({
    highlighter,
    mode,
    visual,
}: {
    /** Passed in when rendering outside the app, where the hook's lazy load can't finish. */
    highlighter?: Highlighter;
    mode: RenderMode;
    visual: VisualDoc;
}) {
    const { syntaxHighlight } = useSettings();
    const loaded = useHighlighter(syntaxHighlight && !highlighter);
    const label = visual.kind === 'plan' ? 'Visual plan' : 'Visual recap';
    const meta = [
        visual.scope,
        visual.commits > 0 && `${visual.commits} commit${visual.commits === 1 ? '' : 's'}`,
        visual.diffs.length > 0 &&
            `${visual.diffs.length} file${visual.diffs.length === 1 ? '' : 's'}`,
        `by ${PROVIDER_LABELS[visual.provider]}`,
        new Date(visual.createdAt * 1000).toLocaleDateString('en', { dateStyle: 'medium' }),
    ].filter(Boolean);

    let previous: null | VisualBlock['type'] = null;

    return (
        <Visual.Provider value={{ highlighter: highlighter ?? loaded, mode }}>
            <article
                className="vz-document flex flex-col"
                onClick={mode === 'app' ? openLink : undefined}
            >
                <header className="mx-auto flex w-full max-w-[880px] flex-col gap-2.5 px-8 pt-8 pb-6">
                    <span className="text-[11px] font-semibold tracking-[0.12em] text-muted uppercase">
                        {label}
                    </span>
                    <h1 className="selectable m-0 text-[30px] leading-tight font-semibold tracking-tight">
                        {visual.title}
                    </h1>
                    {visual.brief && (
                        <p className="selectable m-0 text-[16px] leading-relaxed text-body">
                            <Inline text={visual.brief} />
                        </p>
                    )}
                    {visual.prompt && (
                        <blockquote className="selectable m-0 border-l-2 border-accent-line pl-3 text-[13px] leading-normal whitespace-pre-wrap text-muted">
                            {visual.prompt}
                        </blockquote>
                    )}
                    <span className="font-mono text-[11.5px] text-dim">{meta.join(' · ')}</span>
                    {visual.truncated && (
                        <span className="rounded-md bg-warn-bg px-2.5 py-1.5 text-[12px] text-warn">
                            This change was too large to send whole; the model saw the first part of
                            the diff.
                        </span>
                    )}
                </header>
                {visual.canvas.screens.length > 0 && (
                    <div className={mode === 'app' ? '' : 'mx-auto w-full max-w-[880px] px-8'}>
                        <Canvas canvas={visual.canvas} />
                    </div>
                )}
                <div className="mx-auto flex w-full max-w-[880px] flex-col gap-4 px-8 pt-6 pb-16">
                    {sections(visual.blocks).map((section, index) => {
                        if (section.kind === 'diffs') {
                            previous = 'diff';

                            return (
                                <div className="flex flex-col gap-3" key={index}>
                                    <Heading>Key changes</Heading>
                                    <KeyChanges blocks={section.blocks} diffs={visual.diffs} />
                                </div>
                            );
                        }

                        const { block } = section;
                        const heading = block.type !== previous ? HEADINGS[block.type] : undefined;

                        previous = block.type;

                        return (
                            <div className="flex flex-col gap-3" key={index}>
                                {heading && <Heading>{heading}</Heading>}
                                <BlockView block={block} visual={visual} />
                            </div>
                        );
                    })}
                </div>
            </article>
        </Visual.Provider>
    );
}

function BlockView({ block, visual }: { block: VisualBlock; visual: VisualDoc }) {
    switch (block.type) {
        case 'code':
            return <CodeCard block={block} />;
        case 'compare':
            return <CompareCard block={block} canvas={visual.canvas} kind={visual.kind} />;
        case 'contract':
            return <ContractCard block={block} />;
        case 'decision':
            return <DecisionCard block={block} />;
        case 'diagram':
            return <Diagram diagram={block} />;
        case 'diff':
            return <KeyChanges blocks={[block]} diffs={visual.diffs} />;
        case 'fileTree':
            return <FileTreeCard block={block} />;
        case 'questions':
            return <QuestionsCard block={block} />;
        case 'text':
            return <Prose markdown={block.markdown} />;
        case 'verify':
            return <VerifyCard block={block} />;
    }
}

function openLink(event: MouseEvent<HTMLElement>) {
    const link = (event.target as HTMLElement).closest('a[href]');

    if (!link) {
        return;
    }

    event.preventDefault();

    const href = link.getAttribute('href') ?? '';

    if (/^https?:\/\//.test(href)) {
        void openUrl(href);
    }
}
