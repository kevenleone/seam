import { useId, useMemo } from 'react';

import type { VisualChange, VisualDiagram } from '../../lib/types';

import { layoutDiagram, type PlacedNode } from '../../lib/visual';
import { Inline } from './Markdown';
import { useVisual } from './mode';

const NODE_TONES: Record<VisualChange, { fill: string; stroke: string }> = {
    '': { fill: 'var(--panel)', stroke: 'var(--control-line)' },
    added: { fill: 'var(--add-bg)', stroke: 'var(--add)' },
    modified: { fill: 'var(--accent-bg)', stroke: 'var(--accent)' },
    removed: { fill: 'var(--del-bg)', stroke: 'var(--del)' },
    renamed: { fill: 'var(--panel)', stroke: 'var(--accent-line)' },
};

const EDGE_STROKES: Record<VisualChange, string> = {
    '': 'var(--muted)',
    added: 'var(--add)',
    modified: 'var(--accent)',
    removed: 'var(--del)',
    renamed: 'var(--muted)',
};

export function Diagram({ diagram }: { diagram: VisualDiagram }) {
    const layout = useMemo(() => layoutDiagram(diagram), [diagram]);
    const marker = useId().replace(/:/g, '');
    const { mode } = useVisual();

    return (
        <figure className="m-0 flex flex-col gap-2">
            {diagram.title && (
                <figcaption className="text-[13px] font-semibold">
                    <Inline text={diagram.title} />
                </figcaption>
            )}
            <div className="overflow-x-auto rounded-xl border border-line bg-canvas p-2">
                <svg
                    aria-label={diagram.title || 'Diagram'}
                    className={`mx-auto block h-auto ${mode === 'app' ? '' : 'max-w-full'}`}
                    role="img"
                    style={{ width: layout.width }}
                    viewBox={`0 0 ${layout.width} ${layout.height}`}
                >
                    <defs>
                        {Object.entries(EDGE_STROKES).map(([change, stroke]) => (
                            <marker
                                id={`${marker}-${change || 'none'}`}
                                key={change}
                                markerHeight="8"
                                markerWidth="8"
                                orient="auto-start-reverse"
                                refX="7"
                                refY="4"
                            >
                                <path d="M0,0 L8,4 L0,8 z" fill={stroke} />
                            </marker>
                        ))}
                    </defs>
                    {layout.groups.map((group) => (
                        <g key={group.label}>
                            <rect
                                fill="none"
                                height={group.height}
                                rx="12"
                                stroke="var(--line)"
                                strokeDasharray="4 4"
                                width={group.width}
                                x={group.x}
                                y={group.y}
                            />
                            <text
                                fill="var(--muted)"
                                fontSize="10.5"
                                fontWeight="600"
                                letterSpacing="0.06em"
                                x={group.x + 12}
                                y={group.y + 17}
                            >
                                {group.label.toUpperCase()}
                            </text>
                        </g>
                    ))}
                    {layout.edges.map((edge, index) => (
                        <g key={index}>
                            <path
                                d={edgePath(edge.points)}
                                fill="none"
                                markerEnd={`url(#${marker}-${edge.change || 'none'})`}
                                stroke={EDGE_STROKES[edge.change]}
                                strokeDasharray={edge.change === 'removed' ? '5 4' : undefined}
                                strokeWidth="1.4"
                            />
                            {edge.label && (
                                <text
                                    dominantBaseline="middle"
                                    fill="var(--muted)"
                                    fontSize="11"
                                    paintOrder="stroke"
                                    stroke="var(--canvas)"
                                    strokeWidth="4"
                                    textAnchor="middle"
                                    x={edge.labelX}
                                    y={edge.labelY}
                                >
                                    {edge.label}
                                </text>
                            )}
                        </g>
                    ))}
                    {layout.nodes.map((node) => (
                        <DiagramNode key={node.id} node={node} />
                    ))}
                </svg>
            </div>
        </figure>
    );
}

function DiagramNode({ node }: { node: PlacedNode }) {
    const tone = NODE_TONES[node.change];
    const external = node.kind === 'external';
    const center = node.x + node.width / 2;

    return (
        <g>
            <rect
                fill={tone.fill}
                height={node.height}
                rx={node.kind === 'ui' ? 14 : 7}
                stroke={tone.stroke}
                strokeDasharray={external || node.change === 'removed' ? '5 4' : undefined}
                strokeWidth="1.4"
                width={node.width}
                x={node.x}
                y={node.y}
            />
            {node.kind === 'store' && (
                <line
                    stroke={tone.stroke}
                    strokeWidth="1"
                    x1={node.x}
                    x2={node.x + node.width}
                    y1={node.y + 6}
                    y2={node.y + 6}
                />
            )}
            <text
                fill="var(--fg)"
                fontSize="12.5"
                fontWeight="600"
                textAnchor="middle"
                textDecoration={node.change === 'removed' ? 'line-through' : undefined}
                x={center}
                y={node.y + (node.detail ? 21 : 22)}
            >
                {node.label}
            </text>
            {node.detail && (
                <text
                    fill="var(--muted)"
                    fontSize="10.5"
                    textAnchor="middle"
                    x={center}
                    y={node.y + 38}
                >
                    {node.detail.length > 40 ? `${node.detail.slice(0, 39)}…` : node.detail}
                </text>
            )}
        </g>
    );
}

function edgePath(points: { x: number; y: number }[]): string {
    if (points.length < 3) {
        return points.map((point, index) => `${index ? 'L' : 'M'} ${point.x} ${point.y}`).join(' ');
    }

    // Through the midpoints, bending at each of dagre's points, for a smooth line.
    let path = `M ${points[0].x} ${points[0].y}`;

    for (let index = 1; index < points.length - 1; index += 1) {
        const point = points[index];
        const next = points[index + 1];

        path += ` Q ${point.x} ${point.y} ${(point.x + next.x) / 2} ${(point.y + next.y) / 2}`;
    }

    const last = points.at(-1)!;

    return `${path} L ${last.x} ${last.y}`;
}
