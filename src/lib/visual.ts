import { graphlib, layout } from '@dagrejs/dagre';
import DOMPurify from 'dompurify';
import { marked } from 'marked';

import type { VisualBlock, VisualCanvas, VisualChange, VisualDiagram, VisualScreen } from './types';

// MARK: Sanitising

const LAYOUT_PROPERTIES = new Set([
    'align-items',
    'align-self',
    'display',
    'flex',
    'flex-basis',
    'flex-direction',
    'flex-grow',
    'flex-shrink',
    'flex-wrap',
    'font-size',
    'gap',
    'grid-column',
    'grid-template-columns',
    'height',
    'justify-content',
    'margin',
    'margin-bottom',
    'margin-left',
    'margin-right',
    'margin-top',
    'max-height',
    'max-width',
    'min-height',
    'min-width',
    'padding',
    'padding-bottom',
    'padding-left',
    'padding-right',
    'padding-top',
    'text-align',
    'width',
]);

const SKETCH_CLASS = /^(wf-[a-z-]+|primary|on)$/;

/**
 * Wireframes are the model's HTML. Its own instance, so these hooks never touch the
 * markdown purifier: no scripts, URLs or ids, only the sketch classes, and inline styles
 * cut down to layout so a screen can't cover the window or reach the network.
 */
const sketchPurifier = DOMPurify(window);

sketchPurifier.addHook('uponSanitizeAttribute', (_node, data) => {
    if (data.attrName === 'class') {
        data.attrValue = data.attrValue
            .split(/\s+/)
            .filter((name) => SKETCH_CLASS.test(name))
            .join(' ');
    }

    if (data.attrName === 'style') {
        data.attrValue = data.attrValue
            .split(';')
            .map((declaration) => declaration.trim())
            .filter((declaration) => {
                const [property, value = ''] = declaration.split(':');

                return (
                    LAYOUT_PROPERTIES.has(property.trim().toLowerCase()) &&
                    !/url\(|expression|var\(/i.test(value)
                );
            })
            .join('; ');
    }
});

const SKETCH_CONFIG = {
    ALLOW_DATA_ATTR: true,
    FORBID_ATTR: ['action', 'formaction', 'href', 'id', 'name', 'src', 'srcset', 'xlink:href'],
    FORBID_TAGS: [
        'audio',
        'embed',
        'iframe',
        'img',
        'link',
        'math',
        'object',
        'picture',
        'script',
        'source',
        'style',
        'svg',
        'video',
    ],
};

export function inlineHtml(text: string): string {
    return DOMPurify.sanitize(marked.parseInline(text, { async: false, gfm: true }), {
        FORBID_TAGS: ['form', 'iframe', 'img', 'style'],
    });
}

export function markdownHtml(markdown: string): string {
    return DOMPurify.sanitize(marked.parse(markdown, { async: false, gfm: true }), {
        FORBID_TAGS: ['form', 'iframe', 'img', 'style'],
    });
}

/** A screen's sanitised HTML, with its pins' numbered badges placed on their targets. */
export function sketchHtml(screen: VisualScreen, canvas: VisualCanvas): string {
    const fragment = sketchPurifier.sanitize(screen.html, {
        ...SKETCH_CONFIG,
        RETURN_DOM_FRAGMENT: true,
    });

    for (const pin of canvas.pins.filter((candidate) => candidate.screen === screen.id)) {
        const target = fragment.querySelector(`[data-pin="${CSS.escape(pin.target)}"]`);

        if (!target) {
            continue;
        }

        const badge = document.createElement('span');

        badge.className = 'wf-pin';
        badge.textContent = String(pin.number);
        target.append(badge);
    }

    const holder = document.createElement('div');

    holder.append(fragment);

    return holder.innerHTML;
}

/** The same markup as XHTML, which an SVG `foreignObject` needs. */
export function sketchXhtml(screen: VisualScreen, canvas: VisualCanvas): string {
    const holder = document.createElement('div');

    holder.innerHTML = sketchHtml(screen, canvas);

    return [...holder.childNodes]
        .map((node) => new XMLSerializer().serializeToString(node))
        .join('');
}

// MARK: Wireframe sheet

/**
 * Scoped to `.wf-root`, so it reads the same in the app, a print, an exported page and
 * an Excalidraw image. The `--wf-*` colours come from the theme's tokens.
 */
export const SKETCH_CSS = `
.wf-root {
    --wf-ink: var(--fg-soft);
    --wf-line: var(--muted);
    --wf-fill: var(--app);
    --wf-panel: var(--panel);
    --wf-soft: var(--control-line);
    --wf-accent: var(--accent);
    --wf-on-accent: var(--on-accent);
    --wf-mark: var(--add);
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: 100%;
    height: 100%;
    overflow: hidden;
    padding: 14px;
    background: var(--wf-fill);
    color: var(--wf-ink);
    font-family: 'Patrick Hand', 'Comic Sans MS', cursive;
    font-size: 15px;
    line-height: 1.3;
    text-align: left;
    pointer-events: none;
}
.wf-root * { box-sizing: border-box; }
.wf-root h1, .wf-root h2, .wf-root h3, .wf-root h4, .wf-root p { margin: 0; font-weight: normal; }
.wf-root h1, .wf-root .wf-title { font-size: 23px; }
.wf-root h2 { font-size: 19px; }
.wf-root h3, .wf-root h4 { font-size: 16px; }
.wf-root ul, .wf-root ol { margin: 0; padding-left: 18px; }
.wf-root .wf-row { display: flex; gap: 10px; align-items: center; }
.wf-root .wf-col { display: flex; flex-direction: column; gap: 8px; }
.wf-root .wf-sidebar { display: flex; flex-direction: column; gap: 8px; width: 28%; flex: none; align-self: stretch; padding-right: 10px; border-right: 1.5px solid var(--wf-line); }
.wf-root .wf-nav { display: flex; gap: 12px; align-items: center; padding-bottom: 8px; border-bottom: 1.5px solid var(--wf-line); }
.wf-root .wf-card { display: flex; flex-direction: column; gap: 8px; padding: 10px; border: 1.5px solid var(--wf-line); border-radius: 11px 8px 12px 7px; background: var(--wf-panel); }
.wf-root .wf-muted { color: var(--wf-line); font-size: 13px; }
.wf-root .wf-line { display: block; width: 70%; min-width: 40px; height: 8px; flex: none; border-radius: 4px; background: var(--wf-soft); }
.wf-root .wf-button, .wf-root button { display: inline-flex; align-items: center; justify-content: center; gap: 6px; padding: 3px 12px; border: 1.5px solid var(--wf-ink); border-radius: 8px 6px 9px 7px; background: transparent; color: var(--wf-ink); font: inherit; white-space: nowrap; }
.wf-root .wf-button.primary, .wf-root button.primary { border-color: var(--wf-accent); background: var(--wf-accent); color: var(--wf-on-accent); }
.wf-root .wf-input, .wf-root input, .wf-root textarea, .wf-root .wf-select, .wf-root select { display: block; width: 100%; min-height: 28px; padding: 3px 8px; border: 1.5px solid var(--wf-line); border-radius: 6px 5px 7px 5px; background: var(--wf-fill); color: var(--wf-ink); font: inherit; }
.wf-root .wf-select::after { content: '▾'; float: right; color: var(--wf-line); }
.wf-root .wf-checkbox { display: inline-block; width: 15px; height: 15px; flex: none; border: 1.5px solid var(--wf-ink); border-radius: 3px; }
.wf-root .wf-checkbox.on { border-color: var(--wf-accent); background: var(--wf-accent); }
.wf-root .wf-toggle { position: relative; display: inline-block; width: 32px; height: 18px; flex: none; border: 1.5px solid var(--wf-ink); border-radius: 999px; }
.wf-root .wf-toggle::after { content: ''; position: absolute; top: 2px; left: 2px; width: 11px; height: 11px; border-radius: 50%; background: var(--wf-ink); }
.wf-root .wf-toggle.on { border-color: var(--wf-accent); background: var(--wf-accent); }
.wf-root .wf-toggle.on::after { left: 16px; background: var(--wf-on-accent); }
.wf-root .wf-chip { display: inline-flex; align-items: center; padding: 1px 10px; border: 1.5px solid var(--wf-line); border-radius: 999px; font-size: 13px; }
.wf-root .wf-badge { display: inline-flex; align-items: center; padding: 0 8px; border: 1.5px solid var(--wf-mark); border-radius: 999px; color: var(--wf-mark); font-size: 12px; }
.wf-root .wf-list { display: flex; flex-direction: column; }
.wf-root .wf-item { display: flex; gap: 10px; align-items: center; padding: 7px 0; border-bottom: 1px solid var(--wf-soft); }
.wf-root .wf-avatar { display: inline-block; width: 26px; height: 26px; flex: none; border: 1.5px solid var(--wf-line); border-radius: 50%; }
.wf-root .wf-image { min-height: 64px; border: 1.5px solid var(--wf-line); border-radius: 6px; background: repeating-linear-gradient(135deg, transparent 0 9px, var(--wf-soft) 9px 10px); }
.wf-root .wf-divider, .wf-root hr { width: 100%; height: 0; margin: 2px 0; border: 0; border-top: 1.5px solid var(--wf-line); }
.wf-root .wf-changed { outline: 2px dashed var(--wf-mark); outline-offset: 3px; border-radius: 6px; }
.wf-root [data-pin] { position: relative; }
.wf-root .wf-pin { position: absolute; top: -11px; left: -11px; z-index: 2; display: flex; align-items: center; justify-content: center; width: 22px; height: 22px; border-radius: 50%; background: var(--wf-mark); color: var(--on-accent); font: 600 12px/1 system-ui, sans-serif; box-shadow: 0 0 0 3px var(--add-bg); }
`;

// MARK: Canvas layout

export interface Surface {
    chrome: 'dialog' | 'none' | 'phone' | 'window';
    height: number;
    width: number;
}

const SURFACES: Record<string, Surface> = {
    desktop: { chrome: 'window', height: 470, width: 760 },
    dialog: { chrome: 'dialog', height: 340, width: 520 },
    mobile: { chrome: 'phone', height: 600, width: 300 },
    panel: { chrome: 'none', height: 480, width: 380 },
    popover: { chrome: 'none', height: 240, width: 320 },
};

export const CHROME_HEIGHT = 26;

export interface CanvasLayout {
    connectors: PlacedConnector[];
    height: number;
    screens: PlacedScreen[];
    width: number;
}

export interface PlacedConnector {
    label: null | string;
    path: string;
    x: number;
    y: number;
}

export interface PlacedScreen {
    height: number;
    screen: VisualScreen;
    width: number;
    x: number;
    y: number;
}

export function surfaceOf(screen: VisualScreen): Surface {
    return SURFACES[screen.surface.trim().toLowerCase()] ?? SURFACES.desktop;
}

const BOARD_MARGIN = 48;
const SCREEN_GAP = 200;
const TITLE_SPACE = 34;

export interface DiagramLayout {
    edges: PlacedEdge[];
    groups: PlacedGroup[];
    height: number;
    nodes: PlacedNode[];
    width: number;
}

export interface PlacedEdge {
    change: VisualChange;
    label: null | string;
    labelX: number;
    labelY: number;
    points: { x: number; y: number }[];
}

export interface PlacedGroup {
    height: number;
    label: string;
    width: number;
    x: number;
    y: number;
}

// MARK: Diagram layout

export interface PlacedNode {
    change: VisualChange;
    detail: null | string;
    height: number;
    id: string;
    kind: string;
    label: string;
    width: number;
    x: number;
    y: number;
}

/** Screens left to right, top-aligned, with connectors between them; no measuring needed. */
export function layoutCanvas(canvas: VisualCanvas): CanvasLayout {
    let x = BOARD_MARGIN;
    const top = BOARD_MARGIN + TITLE_SPACE;
    const screens = walkOrder(canvas).map((screen) => {
        const surface = surfaceOf(screen);
        const placed = { height: surface.height, screen, width: surface.width, x, y: top };

        x += surface.width + SCREEN_GAP;

        return placed;
    });
    const byId = new Map(screens.map((placed) => [placed.screen.id, placed]));
    const connectors = canvas.connectors.flatMap((connector): PlacedConnector[] => {
        const from = byId.get(connector.from);
        const to = byId.get(connector.to);

        if (!from || !to || from === to) {
            return [];
        }

        if (to.x > from.x) {
            const start = {
                x: from.x + from.width + 8,
                y: from.y + Math.min(from.height, to.height) / 2,
            };
            const end = { x: to.x - 8, y: start.y };
            const bend = (end.x - start.x) / 2;

            return [
                {
                    label: connector.label,
                    path: `M ${start.x} ${start.y} C ${start.x + bend} ${start.y - 26}, ${end.x - bend} ${end.y + 26}, ${end.x} ${end.y}`,
                    x: (start.x + end.x) / 2,
                    y: start.y,
                },
            ];
        }

        const start = { x: from.x + from.width / 2, y: from.y - TITLE_SPACE - 4 };
        const end = { x: to.x + to.width / 2, y: to.y - TITLE_SPACE - 4 };
        const lift = BOARD_MARGIN - 6;

        return [
            {
                label: connector.label,
                path: `M ${start.x} ${start.y} C ${start.x} ${start.y - lift}, ${end.x} ${end.y - lift}, ${end.x} ${end.y}`,
                x: (start.x + end.x) / 2,
                y: start.y - lift * 0.75,
            },
        ];
    });
    const notes = Math.max(
        0,
        ...screens.map((placed) => notesHeight(placed.screen, canvas, placed.width)),
    );

    return {
        connectors,
        height: top + Math.max(0, ...screens.map((placed) => placed.height)) + notes + BOARD_MARGIN,
        screens,
        width: Math.max(x - SCREEN_GAP + BOARD_MARGIN, 0),
    };
}

/** How much room the notes and pins under a screen take, roughly. */
function notesHeight(screen: VisualScreen, canvas: VisualCanvas, width: number): number {
    const perLine = Math.max(20, Math.floor(width / 7));
    const lines = (text: string) => Math.ceil(text.length / perLine) || 1;
    const pins = canvas.pins.filter((pin) => pin.screen === screen.id);

    return (
        24 +
        (screen.note ? lines(screen.note) * 19 + 8 : 0) +
        pins.reduce((sum, pin) => sum + 10 + lines(`${pin.title} ${pin.body}`) * 18, 0)
    );
}

/** Screens in the order their connectors walk them, then the rest as written. */
function walkOrder(canvas: VisualCanvas): VisualScreen[] {
    const byId = new Map(canvas.screens.map((screen) => [screen.id, screen]));
    const incoming = new Set(canvas.connectors.map((connector) => connector.to));
    const ordered: VisualScreen[] = [];

    const visit = (screen: VisualScreen) => {
        if (ordered.includes(screen)) {
            return;
        }

        ordered.push(screen);

        for (const connector of canvas.connectors.filter((link) => link.from === screen.id)) {
            const next = byId.get(connector.to);

            if (next) {
                visit(next);
            }
        }
    };

    canvas.screens.filter((screen) => !incoming.has(screen.id)).forEach(visit);
    canvas.screens.forEach(visit);

    return ordered;
}

const GROUP_PREFIX = '\u0000group:';

/** Consecutive diff blocks become one set of key-change tabs. */
export type Section =
    | { block: VisualBlock; kind: 'block' }
    | { blocks: Extract<VisualBlock, { type: 'diff' }>[]; kind: 'diffs' };

/** Wider than this, a left-to-right diagram reads better top to bottom. */
const DIAGRAM_WIDTH = 820;
const GROUP_PADDING = 14;
const GROUP_LABEL = 22;

/** Positions from dagre; node and group x/y are their top-left corners. */
export function layoutDiagram(diagram: VisualDiagram): DiagramLayout {
    if (diagram.direction === 'down') {
        return layoutOnce(diagram, 'TB');
    }

    const across = layoutOnce(diagram, 'LR');

    if (across.width <= DIAGRAM_WIDTH) {
        return across;
    }

    const down = layoutOnce(diagram, 'TB');

    return down.width < across.width ? down : across;
}

export function sections(blocks: VisualBlock[]): Section[] {
    const result: Section[] = [];

    for (const block of blocks) {
        const last = result.at(-1);

        if (block.type === 'diff' && last?.kind === 'diffs') {
            last.blocks.push(block);
        } else if (block.type === 'diff') {
            result.push({ blocks: [block], kind: 'diffs' });
        } else {
            result.push({ block, kind: 'block' });
        }
    }

    return result;
}

function layoutOnce(diagram: VisualDiagram, rankdir: 'LR' | 'TB'): DiagramLayout {
    const graph = new graphlib.Graph({ compound: true, multigraph: true });

    // An edge label takes a rank of its own, so top to bottom needs far less space per rank.
    graph.setGraph({
        marginx: 20,
        marginy: 20,
        nodesep: 30,
        rankdir,
        ranksep: rankdir === 'TB' ? 22 : 70,
    });
    graph.setDefaultEdgeLabel(() => ({}));

    const groups = [
        ...new Set(diagram.nodes.map((node) => node.group?.trim()).filter((group) => !!group)),
    ] as string[];

    for (const group of groups) {
        graph.setNode(GROUP_PREFIX + group, { label: group });
    }

    for (const node of diagram.nodes) {
        graph.setNode(node.id, { ...nodeSize(node.label, node.detail), label: node.label });

        if (node.group?.trim()) {
            graph.setParent(node.id, GROUP_PREFIX + node.group.trim());
        }
    }

    diagram.edges.forEach((edge, index) =>
        graph.setEdge(
            edge.from,
            edge.to,
            {
                height: edge.label ? 16 : 0,
                labelpos: 'c',
                width: edge.label ? edge.label.length * 6.4 + 12 : 0,
            },
            String(index),
        ),
    );

    layout(graph);

    const nodes = diagram.nodes.map((node): PlacedNode => {
        const placed = graph.node(node.id);

        return {
            change: node.change,
            detail: node.detail?.replace(/`/g, '') ?? null,
            height: placed.height,
            id: node.id,
            kind: node.kind,
            label: node.label.replace(/`/g, ''),
            width: placed.width,
            x: placed.x - placed.width / 2,
            y: placed.y - placed.height / 2,
        };
    });
    const edges = diagram.edges.map((edge, index): PlacedEdge => {
        const placed = graph.edge({ name: String(index), v: edge.from, w: edge.to });

        return {
            change: edge.change,
            label: edge.label?.replace(/`/g, '') ?? null,
            labelX: placed.x ?? 0,
            labelY: placed.y ?? 0,
            points: placed.points ?? [],
        };
    });
    // Drawn around the members rather than from dagre's cluster sizes, which leave
    // members outside the box and no room for the label.
    const placedGroups = groups.flatMap((group): PlacedGroup[] => {
        const members = nodes.filter(
            (node) => diagram.nodes.find((each) => each.id === node.id)?.group?.trim() === group,
        );

        if (!members.length) {
            return [];
        }

        const left = Math.min(...members.map((node) => node.x)) - GROUP_PADDING;
        const top = Math.min(...members.map((node) => node.y)) - GROUP_PADDING - GROUP_LABEL;
        const right = Math.max(...members.map((node) => node.x + node.width)) + GROUP_PADDING;
        const bottom = Math.max(...members.map((node) => node.y + node.height)) + GROUP_PADDING;

        return [{ height: bottom - top, label: group, width: right - left, x: left, y: top }];
    });
    const shapes = [...nodes, ...placedGroups];
    const shiftX = Math.max(0, 20 - Math.min(20, ...shapes.map((shape) => shape.x)));
    const shiftY = Math.max(0, 20 - Math.min(20, ...shapes.map((shape) => shape.y)));
    const move = <T extends { x: number; y: number }>(shape: T): T => ({
        ...shape,
        x: shape.x + shiftX,
        y: shape.y + shiftY,
    });

    return {
        edges: edges.map((edge) => ({
            ...edge,
            labelX: edge.labelX + shiftX,
            labelY: edge.labelY + shiftY,
            points: edge.points.map(move),
        })),
        groups: placedGroups.map(move),
        height: Math.max(...shapes.map((shape) => shape.y + shape.height)) + shiftY + 20,
        nodes: nodes.map(move),
        width: Math.max(...shapes.map((shape) => shape.x + shape.width)) + shiftX + 20,
    };
}

function nodeSize(label: string, detail: null | string): { height: number; width: number } {
    const width = Math.max(label.length * 7.6, (detail?.length ?? 0) * 6.1) + 30;

    return { height: detail ? 52 : 36, width: Math.min(Math.max(width, 110), 250) };
}

export const CHANGE_LABELS: Record<Exclude<VisualChange, ''>, string> = {
    added: 'Added',
    modified: 'Modified',
    removed: 'Removed',
    renamed: 'Renamed',
};

/** Adds the wireframe sheet to the document once; an export copies it with the rest. */
export function installSketchStyle(): void {
    if (document.getElementById('wf-sketch')) {
        return;
    }

    const style = document.createElement('style');

    style.id = 'wf-sketch';
    style.textContent = SKETCH_CSS;
    document.head.append(style);
}
