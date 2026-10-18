import { graphlib, layout } from '@dagrejs/dagre';
import DOMPurify from 'dompurify';
import { marked } from 'marked';

export { default as SKETCH_CSS } from './sketch.css?raw';

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

export interface Surface {
    chrome: 'dialog' | 'none' | 'phone' | 'window';
    height: number;
    width: number;
}

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

// MARK: Canvas layout

/** The same markup as XHTML, which an SVG `foreignObject` needs. */
export function sketchXhtml(screen: VisualScreen, canvas: VisualCanvas): string {
    const holder = document.createElement('div');

    holder.innerHTML = sketchHtml(screen, canvas);

    return [...holder.childNodes]
        .map((node) => new XMLSerializer().serializeToString(node))
        .join('');
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
