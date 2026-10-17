import { createElement } from 'react';

import type { FileDiff, Visual, VisualBlock, VisualChange, VisualDiagram } from './types';

import { VisualDocument } from '../components/visual/VisualDocument';
import { loadHighlighter } from './useHighlighter';
import {
    CHANGE_LABELS,
    CHROME_HEIGHT,
    layoutCanvas,
    layoutDiagram,
    SKETCH_CSS,
    sketchXhtml,
} from './visual';

export type ExportFormat = 'excalidraw' | 'html' | 'markdown';

export const EXTENSIONS: Record<ExportFormat, string> = {
    excalidraw: 'excalidraw',
    html: 'html',
    markdown: 'md',
};

export function exportName(visual: Visual): string {
    const slug = visual.title
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, '-')
        .replace(/^-|-$/g, '')
        .slice(0, 60);

    return slug || (visual.kind === 'plan' ? 'visual-plan' : 'visual-recap');
}

export function exportVisual(visual: Visual, format: ExportFormat): Promise<string> {
    switch (format) {
        case 'excalidraw':
            return Promise.resolve(toExcalidraw(visual));
        case 'html':
            return toHtml(visual);
        case 'markdown':
            return Promise.resolve(toMarkdown(visual));
    }
}

// MARK: HTML

/**
 * The app's stylesheets as one string, with only the Latin font files kept and inlined,
 * so the page needs nothing from the network.
 */
async function collectCss(): Promise<string> {
    const parts: string[] = [];

    for (const sheet of [...document.styleSheets]) {
        let rules: CSSRule[];

        try {
            rules = [...sheet.cssRules];
        } catch {
            continue;
        }

        for (const rule of rules) {
            if (!(rule instanceof CSSFontFaceRule)) {
                parts.push(rule.cssText);
                continue;
            }

            const url = /url\("?([^")]+\.woff2)"?\)/.exec(rule.style.getPropertyValue('src'))?.[1];

            if (!url || !/-latin-\d+-normal\.woff2/.test(url)) {
                continue;
            }

            try {
                const inlined = await dataUrl(await (await fetch(url)).blob());

                parts.push(
                    `@font-face { font-family: ${rule.style.getPropertyValue('font-family')}; font-style: ${rule.style.getPropertyValue('font-style') || 'normal'}; font-weight: ${rule.style.getPropertyValue('font-weight') || '400'}; font-display: swap; src: url(${inlined}) format('woff2'); }`,
                );
            } catch {
                // The system font stands in.
            }
        }
    }

    return parts.join('\n');
}

function dataUrl(blob: Blob): Promise<string> {
    return new Promise((resolve, reject) => {
        const reader = new FileReader();

        reader.onload = () => resolve(String(reader.result));
        reader.onerror = () => reject(reader.error);
        reader.readAsDataURL(blob);
    });
}

function escapeHtml(text: string): string {
    return text
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;');
}

/** Brings back pan and zoom on an exported page's wireframe boards. */
const CANVAS_SCRIPT = `
document.querySelectorAll('.vz-canvas').forEach(function (canvas) {
    var board = canvas.querySelector('.vz-board');
    var label = canvas.querySelector('.vz-zoom-label');
    var width = Number(canvas.dataset.width), height = Number(canvas.dataset.height);
    var view = { x: 0, y: 0, zoom: 1 }, drag = null;
    function apply() {
        board.style.transform = 'translate(' + view.x + 'px,' + view.y + 'px) scale(' + view.zoom + ')';
        if (label) label.textContent = Math.round(view.zoom * 100) + '%';
    }
    function fit() {
        view = { x: 0, y: 0, zoom: Math.min(1, canvas.clientWidth / width) };
        canvas.style.height = Math.max(240, Math.min(height * view.zoom, 640)) + 'px';
        apply();
    }
    function zoomBy(factor) {
        var zoom = Math.min(2, Math.max(0.2, view.zoom * factor)), cx = canvas.clientWidth / 2, cy = canvas.clientHeight / 2;
        view = { x: cx - (cx - view.x) * zoom / view.zoom, y: cy - (cy - view.y) * zoom / view.zoom, zoom: zoom };
        apply();
    }
    canvas.addEventListener('click', function (event) {
        var action = event.target.closest('[data-zoom]');
        if (!action) return;
        var kind = action.dataset.zoom;
        if (kind === 'fit') fit(); else zoomBy(kind === 'in' ? 1.25 : 0.8);
    });
    canvas.addEventListener('pointerdown', function (event) {
        if (event.target.closest('button')) return;
        drag = { x: event.clientX, y: event.clientY };
        canvas.setPointerCapture(event.pointerId);
        canvas.style.cursor = 'grabbing';
    });
    canvas.addEventListener('pointermove', function (event) {
        if (!drag) return;
        view.x += event.clientX - drag.x; view.y += event.clientY - drag.y;
        drag = { x: event.clientX, y: event.clientY };
        apply();
    });
    canvas.addEventListener('pointerup', function () { drag = null; canvas.style.cursor = 'grab'; });
    canvas.addEventListener('wheel', function (event) {
        if (!event.ctrlKey && !event.metaKey) return;
        event.preventDefault();
        zoomBy(Math.exp(-event.deltaY / 200));
    }, { passive: false });
    canvas.style.cursor = 'grab';
    fit();
    window.addEventListener('resize', fit);
});
`;

async function toHtml(visual: Visual): Promise<string> {
    const [{ renderToStaticMarkup }, highlighter, css] = await Promise.all([
        import('react-dom/server'),
        loadHighlighter(),
        collectCss(),
    ]);
    const body = renderToStaticMarkup(
        createElement(VisualDocument, { highlighter, mode: 'export', visual }),
    );
    const root = document.documentElement;
    const policy =
        "default-src 'none'; style-src 'unsafe-inline'; font-src data:; img-src data:; script-src 'unsafe-inline'";

    return `<!doctype html>
<html lang="en" data-theme="${escapeHtml(root.dataset.theme ?? 'dark')}" style="${escapeHtml(root.style.cssText)}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta http-equiv="Content-Security-Policy" content="${policy}">
<meta name="generator" content="Seam">
<title>${escapeHtml(visual.title)}</title>
<style>${css}</style>
<style>html, body { margin: 0; background: var(--app); color: var(--fg); font-family: 'IBM Plex Sans', system-ui, sans-serif; font-size: 13px; } .vz-prose, .selectable { user-select: text; }</style>
</head>
<body>
${body}
<script>${CANVAS_SCRIPT}</script>
</body>
</html>
`;
}

// MARK: Markdown

const CHANGE_LETTERS: Record<Exclude<VisualChange, ''>, string> = {
    added: 'A',
    modified: 'M',
    removed: 'D',
    renamed: 'R',
};

/** The diagram as a Mermaid flowchart, which GitHub and most editors draw. */
export function toMermaid(diagram: VisualDiagram): string {
    const ids = new Map(diagram.nodes.map((node, index) => [node.id, mermaidId(node.id, index)]));
    const lines = [`flowchart ${diagram.direction === 'down' ? 'TD' : 'LR'}`];
    const groups = new Map<string, string[]>();

    for (const node of diagram.nodes) {
        const label = node.detail
            ? `${mermaidText(node.label)}<br/><small>${mermaidText(node.detail)}</small>`
            : mermaidText(node.label);
        const line = `${ids.get(node.id)}["${label}"]`;
        const group = node.group?.trim();

        if (group) {
            groups.set(group, [...(groups.get(group) ?? []), line]);
        } else {
            lines.push(`    ${line}`);
        }
    }

    [...groups].forEach(([group, members], index) => {
        lines.push(
            `    subgraph g${index}[${mermaidLabel(group)}]`,
            ...members.map((line) => `        ${line}`),
            '    end',
        );
    });

    for (const edge of diagram.edges) {
        const arrow = edge.change === 'removed' ? '-.->' : '-->';
        const label = edge.label ? `|${mermaidLabel(edge.label)}|` : '';

        lines.push(`    ${ids.get(edge.from)} ${arrow}${label} ${ids.get(edge.to)}`);
    }

    lines.push(
        '    classDef added fill:#d0ebff,stroke:#1971c2',
        '    classDef modified fill:#e5dbff,stroke:#6741d9',
        '    classDef removed fill:#ffe8cc,stroke:#e8590c,stroke-dasharray:4',
    );

    for (const change of ['added', 'modified', 'removed'] as const) {
        const members = diagram.nodes.filter((node) => node.change === change);

        if (members.length) {
            lines.push(`    class ${members.map((node) => ids.get(node.id)).join(',')} ${change}`);
        }
    }

    return fence(lines.join('\n'), 'mermaid');
}

function blockMarkdown(block: VisualBlock, visual: Visual): string {
    const afterLabel = visual.kind === 'plan' ? 'Proposed' : 'After';

    switch (block.type) {
        case 'code':
            return [
                `### ${block.file || block.language}`,
                block.summary,
                fence(block.code, block.language),
                ...block.annotations.map(
                    (annotation) => `- Line ${annotation.line ?? '?'}: ${annotation.note}`,
                ),
            ]
                .filter(Boolean)
                .join('\n\n');

        case 'compare': {
            const side = (label: string, value: typeof block.before) => {
                const screen = value.screen
                    ? visual.canvas.screens.find((each) => each.id === value.screen)
                    : null;
                const content = screen
                    ? `_Screen: ${screen.title}_${screen.note ? ` — ${screen.note}` : ''}`
                    : value.code
                      ? fence(value.code, value.language ?? '')
                      : (value.text ?? '');

                return `**${label}**\n\n${content}`;
            };

            return [
                block.title && `### ${block.title}`,
                side('Before', block.before),
                side(afterLabel, block.after),
            ]
                .filter(Boolean)
                .join('\n\n');
        }

        case 'contract':
            return [
                block.title && `### ${block.title}`,
                block.endpoints.length > 0 &&
                    [
                        '| Method | Path | Change | Summary |',
                        '| --- | --- | --- | --- |',
                        ...block.endpoints.map(
                            (endpoint) =>
                                `| ${endpoint.method} | \`${endpoint.path}\` | ${endpoint.change ? CHANGE_LABELS[endpoint.change] : ''} | ${endpoint.summary.replace(/\|/g, '\\|')} |`,
                        ),
                    ].join('\n'),
                ...block.entities.map((entity) =>
                    [
                        `**${entity.name}**${entity.change ? ` (${CHANGE_LABELS[entity.change].toLowerCase()})` : ''}`,
                        ...entity.fields.map(
                            (field) =>
                                `- \`${field.name}\`: ${field.was ? `~~${field.was}~~ → ` : ''}${field.type}${field.change ? ` _${field.change}_` : ''}`,
                        ),
                    ].join('\n'),
                ),
            ]
                .filter(Boolean)
                .join('\n\n');
        case 'decision':
            return [
                `### Decision: ${block.title}`,
                block.context && `**Context.** ${block.context}`,
                block.decision && `**Decision.** ${block.decision}`,
                block.alternatives.length > 0 &&
                    `**Alternatives considered**\n\n${block.alternatives.map((alternative) => `- ${alternative.option}${alternative.whyNot ? ` — ${alternative.whyNot}` : ''}`).join('\n')}`,
                block.consequences && `**Consequences.** ${block.consequences}`,
            ]
                .filter(Boolean)
                .join('\n\n');
        case 'diagram':
            return [block.title && `### ${block.title}`, toMermaid(block)]
                .filter(Boolean)
                .join('\n\n');

        case 'diff': {
            const diff = visual.diffs.find((each) => (each.newPath ?? each.oldPath) === block.file);

            return [
                `### \`${block.file}\``,
                block.summary,
                ...block.annotations.map(
                    (annotation) =>
                        `- ${annotation.side === 'before' ? 'Old line' : 'Line'} ${annotation.line ?? '?'}: ${annotation.note}`,
                ),
                diff && diffExcerpt(diff),
            ]
                .filter(Boolean)
                .join('\n\n');
        }

        case 'fileTree':
            return [...block.files]
                .sort((first, second) => first.path.localeCompare(second.path))
                .map(
                    (file) =>
                        `- ${file.change ? `**${CHANGE_LETTERS[file.change]}** ` : ''}\`${file.path}\`${file.additions || file.deletions ? ` (+${file.additions} −${file.deletions})` : ''}${file.note ? ` — ${file.note}` : ''}`,
                )
                .join('\n');
        case 'questions':
            return block.items
                .map(
                    (item, index) =>
                        `${index + 1}. **${item.question}**${item.recommendation ? `\n   Recommended: ${item.recommendation}` : ''}`,
                )
                .join('\n');
        case 'text':
            return block.markdown;
        case 'verify':
            return block.steps
                .map(
                    (step) =>
                        `- [ ] **${step.title}**${step.how ? ` ${step.how}` : ''}${step.expect ? `\n      Expect: ${step.expect}` : ''}`,
                )
                .join('\n');
    }
}

function diffExcerpt(diff: FileDiff, limit = 160): string {
    const lines: string[] = [];

    for (const hunk of diff.hunks) {
        lines.push(
            `@@ -${hunk.oldStart},${hunk.oldCount} +${hunk.newStart},${hunk.newCount} @@ ${hunk.section}`.trimEnd(),
        );

        for (const line of hunk.lines) {
            const sign =
                line.kind === 'addition'
                    ? '+'
                    : line.kind === 'deletion'
                      ? '-'
                      : line.kind === 'noNewline'
                        ? '\\'
                        : ' ';

            lines.push(`${sign}${line.text}`);
        }
    }

    const cut =
        lines.length > limit
            ? [...lines.slice(0, limit), `… ${lines.length - limit} more lines`]
            : lines;

    return fence(cut.join('\n'), 'diff');
}

function fence(code: string, language = ''): string {
    const longest = Math.max(2, ...[...code.matchAll(/`+/g)].map((match) => match[0].length));
    const ticks = '`'.repeat(longest + 1);

    return `${ticks}${language}\n${code.replace(/\n$/, '')}\n${ticks}`;
}

function mermaidId(id: string, index: number): string {
    return `n${index}_${id.replace(/[^A-Za-z0-9_]/g, '_')}`;
}

function mermaidLabel(text: string): string {
    return `"${mermaidText(text)}"`;
}

/** Mermaid's own entity codes, so a label like `<form>` stays text rather than markup. */
function mermaidText(text: string): string {
    return text
        .replace(/`/g, '')
        .replace(/"/g, '#quot;')
        .replace(/</g, '#lt;')
        .replace(/>/g, '#gt;');
}

const MARKDOWN_HEADINGS: Partial<Record<VisualBlock['type'], string>> = {
    contract: 'Contracts',
    decision: 'Decisions',
    diagram: 'Architecture',
    diff: 'Key changes',
    fileTree: 'Files',
    questions: 'Open questions',
    verify: 'How to test it',
};

type Element = { boundElements: { id: string; type: string }[] | null; id: string } & Record<
    string,
    unknown
>;

// MARK: Excalidraw

export function toMarkdown(visual: Visual): string {
    const parts = [
        `# ${visual.title}`,
        visual.brief,
        visual.prompt &&
            visual.prompt
                .split('\n')
                .map((line) => `> ${line}`)
                .join('\n'),
        `_${visual.kind === 'plan' ? 'Visual plan' : 'Visual recap'} · ${visual.scope} · ${new Date(visual.createdAt * 1000).toLocaleDateString('en', { dateStyle: 'medium' })}_`,
    ];

    if (visual.canvas.screens.length) {
        parts.push('## Screens');

        for (const placed of layoutCanvas(visual.canvas).screens) {
            const { screen } = placed;
            const state = screen.state
                ? `${screen.state[0].toUpperCase()}${screen.state.slice(1)}: `
                : '';
            const pins = visual.canvas.pins
                .filter((pin) => pin.screen === screen.id)
                .map((pin) => `${pin.number}. **${pin.title}**${pin.body ? ` — ${pin.body}` : ''}`);

            parts.push(
                [`### ${state}${screen.title || screen.id}`, screen.note, pins.join('\n')]
                    .filter(Boolean)
                    .join('\n\n'),
            );
        }

        for (const connector of visual.canvas.connectors) {
            const title = (id: string) =>
                visual.canvas.screens.find((screen) => screen.id === id)?.title ?? id;

            parts.push(
                `- ${title(connector.from)} → ${title(connector.to)}${connector.label ? ` (${connector.label})` : ''}`,
            );
        }
    }

    let previous: null | VisualBlock['type'] = null;

    for (const block of visual.blocks) {
        const heading = block.type !== previous ? MARKDOWN_HEADINGS[block.type] : undefined;

        if (heading) {
            parts.push(`## ${heading}`);
        }

        previous = block.type;
        parts.push(blockMarkdown(block, visual));
    }

    return `${parts.filter(Boolean).join('\n\n')}\n`;
}

const INK = '#1e1e1e';
const MUTED = '#868e96';
const TONES: Record<VisualChange, { fill: string; stroke: string }> = {
    '': { fill: '#ffffff', stroke: INK },
    added: { fill: '#d0ebff', stroke: '#1971c2' },
    modified: { fill: '#e5dbff', stroke: '#6741d9' },
    removed: { fill: '#ffe8cc', stroke: '#e8590c' },
    renamed: { fill: '#f1f3f5', stroke: MUTED },
};
const EXCALIFONT = 5;

let counter = 0;

/**
 * An Excalidraw scene: each screen a frame holding its sketch as an image, with its notes,
 * pins and connectors drawn around it, then every diagram as shapes and bound arrows.
 */
export function toExcalidraw(visual: Visual): string {
    const elements: Element[] = [];
    const files: Record<
        string,
        { created: number; dataURL: string; id: string; mimeType: string }
    > = {};

    elements.push(text(visual.title, 0, -140, { fontSize: 32 }));

    if (visual.brief) {
        elements.push(text(visual.brief, 0, -90, { fontSize: 16, strokeColor: MUTED, width: 900 }));
    }

    const canvas = layoutCanvas(visual.canvas);

    for (const placed of canvas.screens) {
        const { height, screen, width, x, y } = placed;
        const index = visual.canvas.screens.indexOf(screen);
        const frame = element('frame', x, y, width, height, {
            name: `${screen.state ? `${screen.state}: ` : ''}${screen.title || screen.id}`,
        });
        const fileId = nextId();
        const contentHeight = height - CHROME_HEIGHT;

        files[fileId] = {
            created: Date.now(),
            dataURL: `data:image/svg+xml;base64,${base64(screenSvg(visual, index, width, contentHeight))}`,
            id: fileId,
            mimeType: 'image/svg+xml',
        };
        elements.push(
            frame,
            element('rectangle', x, y, width, CHROME_HEIGHT, {
                backgroundColor: '#f1f3f5',
                frameId: frame.id,
                strokeColor: MUTED,
            }),
            element('image', x, y + CHROME_HEIGHT, width, contentHeight, {
                fileId,
                frameId: frame.id,
                scale: [1, 1],
                status: 'saved',
                strokeColor: 'transparent',
            }),
        );

        const pins = visual.canvas.pins.filter((pin) => pin.screen === screen.id);
        const notes = [
            screen.note,
            ...pins.map((pin) => `(${pin.number}) ${pin.title}${pin.body ? ` — ${pin.body}` : ''}`),
        ].filter(Boolean);

        if (notes.length) {
            elements.push(text(notes.join('\n\n'), x, y + height + 18, { fontSize: 15, width }));
        }
    }

    for (const connector of visual.canvas.connectors) {
        const from = canvas.screens.find((placed) => placed.screen.id === connector.from);
        const to = canvas.screens.find((placed) => placed.screen.id === connector.to);

        if (!from || !to) {
            continue;
        }

        const start = { x: from.x + from.width + 8, y: from.y + from.height / 2 };
        const end =
            to.x > from.x ? { x: to.x - 8, y: start.y } : { x: to.x + to.width / 2, y: to.y - 12 };

        elements.push(arrow([start, end], { strokeColor: '#1971c2', strokeStyle: 'dashed' }));

        if (connector.label) {
            elements.push(
                text(
                    connector.label,
                    (start.x + end.x) / 2 - connector.label.length * 4,
                    start.y - 28,
                    { fontSize: 14, strokeColor: '#1971c2' },
                ),
            );
        }
    }

    let top = canvas.screens.length ? canvas.height + 120 : 0;

    for (const block of visual.blocks) {
        if (block.type === 'diagram') {
            top = addDiagram(block, top, elements) + 120;
        }
    }

    return JSON.stringify(
        {
            appState: { gridSize: 20, viewBackgroundColor: '#ffffff' },
            elements,
            files,
            source: 'https://github.com/kevenleone/seam',
            type: 'excalidraw',
            version: 2,
        },
        null,
        2,
    );
}

function addDiagram(diagram: VisualDiagram, top: number, elements: Element[]): number {
    const layout = layoutDiagram(diagram);
    const left = 0;
    const nodes = new Map<string, Element>();

    if (diagram.title) {
        elements.push(text(diagram.title, left, top, { fontSize: 20 }));
    }

    const offset = top + (diagram.title ? 40 : 0);

    for (const group of layout.groups) {
        elements.push(
            element('rectangle', left + group.x, offset + group.y, group.width, group.height, {
                roundness: { type: 3 },
                strokeColor: MUTED,
                strokeStyle: 'dashed',
            }),
            text(group.label.toUpperCase(), left + group.x + 12, offset + group.y + 8, {
                fontSize: 12,
                strokeColor: MUTED,
            }),
        );
    }

    for (const node of layout.nodes) {
        const tone = TONES[node.change];
        const box = element('rectangle', left + node.x, offset + node.y, node.width, node.height, {
            backgroundColor: tone.fill,
            roundness: { type: 3 },
            strokeColor: tone.stroke,
            strokeStyle: node.kind === 'external' || node.change === 'removed' ? 'dashed' : 'solid',
        });
        const label = text(node.detail ? `${node.label}\n${node.detail}` : node.label, 0, 0, {
            containerId: box.id,
            fontSize: node.detail ? 14 : 16,
            textAlign: 'center',
            verticalAlign: 'middle',
        });

        label.x = (box.x as number) + (node.width - (label.width as number)) / 2;
        label.y = (box.y as number) + (node.height - (label.height as number)) / 2;
        box.boundElements = [{ id: label.id, type: 'text' }];
        nodes.set(node.id, box);
        elements.push(box, label);
    }

    diagram.edges.forEach((edge, index) => {
        const placed = layout.edges[index];
        const from = nodes.get(edge.from);
        const to = nodes.get(edge.to);

        if (!placed?.points.length || !from || !to) {
            return;
        }

        const line = arrow(
            placed.points.map((point) => ({ x: left + point.x, y: offset + point.y })),
            {
                endBinding: { elementId: to.id, focus: 0, gap: 4 },
                startBinding: { elementId: from.id, focus: 0, gap: 4 },
                strokeColor: TONES[edge.change].stroke === INK ? MUTED : TONES[edge.change].stroke,
                strokeStyle: edge.change === 'removed' ? 'dashed' : 'solid',
            },
        );

        from.boundElements = [...(from.boundElements ?? []), { id: line.id, type: 'arrow' }];
        to.boundElements = [...(to.boundElements ?? []), { id: line.id, type: 'arrow' }];
        elements.push(line);

        if (edge.label) {
            elements.push(
                text(
                    edge.label,
                    left + placed.labelX - edge.label.length * 3.5,
                    offset + placed.labelY - 9,
                    { fontSize: 13, strokeColor: MUTED },
                ),
            );
        }
    });

    return offset + layout.height;
}

function arrow(points: { x: number; y: number }[], extra: Record<string, unknown> = {}): Element {
    const [first] = points;
    const relative = points.map((point) => [point.x - first.x, point.y - first.y]);
    const xs = relative.map(([x]) => x);
    const ys = relative.map(([, y]) => y);

    return element(
        'arrow',
        first.x,
        first.y,
        Math.max(...xs) - Math.min(...xs),
        Math.max(...ys) - Math.min(...ys),
        {
            endArrowhead: 'arrow',
            endBinding: null,
            lastCommittedPoint: null,
            points: relative,
            roundness: { type: 2 },
            startArrowhead: null,
            startBinding: null,
            ...extra,
        },
    );
}

function base64(textContent: string): string {
    const bytes = new TextEncoder().encode(textContent);
    let binary = '';

    for (const byte of bytes) {
        binary += String.fromCharCode(byte);
    }

    return btoa(binary);
}

function element(
    type: string,
    left: number,
    top: number,
    width: number,
    height: number,
    extra: Record<string, unknown> = {},
): Element {
    return {
        angle: 0,
        backgroundColor: 'transparent',
        boundElements: null,
        fillStyle: 'solid',
        frameId: null,
        groupIds: [],
        height,
        id: nextId(),
        isDeleted: false,
        link: null,
        locked: false,
        opacity: 100,
        roughness: 1,
        roundness: null,
        seed: seed(),
        strokeColor: INK,
        strokeStyle: 'solid',
        strokeWidth: 1,
        type,
        updated: Date.now(),
        version: 1,
        versionNonce: seed(),
        width,
        x: left,
        y: top,
        ...extra,
    };
}

function nextId(): string {
    counter += 1;

    return `seam-${Date.now().toString(36)}-${counter}`;
}

/** The sheet with every `var(--token)` replaced by its colour, since an image has no theme. */
function resolvedSketchCss(): string {
    const styles = getComputedStyle(document.documentElement);

    return SKETCH_CSS.replace(/var\((--[a-z-]+)\)/g, (match, name: string) => {
        const own = /--wf-[a-z-]+/.test(name) ? null : styles.getPropertyValue(name).trim();

        return own || match;
    });
}

function screenSvg(visual: Visual, screenIndex: number, width: number, height: number): string {
    const screen = visual.canvas.screens[screenIndex];
    const styles = getComputedStyle(document.documentElement);
    const token = (name: string) => styles.getPropertyValue(name).trim();

    return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}"><foreignObject x="0" y="0" width="${width}" height="${height}"><div xmlns="http://www.w3.org/1999/xhtml" style="width:${width}px;height:${height}px;background:${token('--app')}"><style>${resolvedSketchCss().replace(/</g, '\\3c ')}</style><div class="wf-root">${sketchXhtml(screen, visual.canvas)}</div></div></foreignObject></svg>`;
}

function seed(): number {
    return Math.floor(Math.random() * 2 ** 31);
}

function text(
    content: string,
    left: number,
    top: number,
    options: { fontSize?: number; width?: number } & Record<string, unknown> = {},
): Element {
    const fontSize = options.fontSize ?? 16;
    const perLine = options.width
        ? Math.max(8, Math.floor(options.width / (fontSize * 0.55)))
        : Infinity;
    const wrapped = content
        .split('\n')
        .flatMap((line) => {
            if (line.length <= perLine) {
                return [line];
            }

            const words = line.split(' ');
            const result: string[] = [];
            let current = '';

            for (const word of words) {
                if (current && `${current} ${word}`.length > perLine) {
                    result.push(current);
                    current = word;
                } else {
                    current = current ? `${current} ${word}` : word;
                }
            }

            return [...result, current];
        })
        .join('\n');
    const lines = wrapped.split('\n');
    const width = options.width ?? Math.max(...lines.map((line) => line.length)) * fontSize * 0.55;

    return element('text', left, top, width, lines.length * fontSize * 1.25, {
        autoResize: !options.width,
        containerId: null,
        fontFamily: EXCALIFONT,
        fontSize,
        lineHeight: 1.25,
        originalText: wrapped,
        text: wrapped,
        textAlign: 'left',
        verticalAlign: 'top',
        ...options,
    });
}
