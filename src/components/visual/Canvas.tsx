import { useMemo } from 'react';

import type { VisualCanvas, VisualScreen } from '../../lib/types';

import {
    type CanvasLayout,
    CHROME_HEIGHT,
    layoutCanvas,
    sketchHtml,
    surfaceOf,
} from '../../lib/visual';
import { Board } from './Board';
import { Inline } from './Markdown';
import { useVisual } from './mode';

const PRINT_WIDTH = 720;

const STATE_LABELS = { after: 'After', before: 'Before', proposed: 'Proposed' };

/** The wireframe board: screens left to right, connectors between them, pins and notes. */
export function Canvas({ canvas, framed = true }: { canvas: VisualCanvas; framed?: boolean }) {
    const { mode } = useVisual();
    const layout = useMemo(() => layoutCanvas(canvas), [canvas]);

    if (!layout.screens.length) {
        return null;
    }

    if (mode === 'app') {
        return <CanvasBoard canvas={canvas} framed={framed} layout={layout} />;
    }

    const scale = Math.min(1, PRINT_WIDTH / layout.width);

    return (
        <div
            className="vz-canvas relative overflow-hidden rounded-xl border border-line bg-canvas"
            data-height={layout.height}
            data-width={layout.width}
            style={{ height: layout.height * scale }}
        >
            <div
                className="vz-board absolute top-0 left-0 origin-top-left"
                style={{ height: layout.height, transform: `scale(${scale})`, width: layout.width }}
            >
                <BoardContent canvas={canvas} layout={layout} />
            </div>
            {mode === 'export' && (
                <div className="vz-zoom absolute bottom-3 left-3 flex items-center gap-1 rounded-lg border border-line bg-panel px-1 py-0.5 text-[12px] shadow-sm">
                    <button className="px-1.5" data-zoom="out" type="button">
                        −
                    </button>
                    <span className="vz-zoom-label w-11 text-center font-mono">
                        {Math.round(scale * 100)}%
                    </span>
                    <button className="px-1.5" data-zoom="in" type="button">
                        +
                    </button>
                    <button className="px-1.5 text-muted" data-zoom="fit" type="button">
                        Fit
                    </button>
                </div>
            )}
        </div>
    );
}

/** One artboard: its device chrome and the sketch inside, at its surface's full size. */
export function ScreenFrame({ canvas, screen }: { canvas: VisualCanvas; screen: VisualScreen }) {
    const surface = surfaceOf(screen);
    const html = useMemo(() => sketchHtml(screen, canvas), [screen, canvas]);
    const radius = surface.chrome === 'phone' ? 'rounded-[26px]' : 'rounded-[10px]';

    return (
        <div
            className={`flex h-full w-full flex-col overflow-hidden border-[1.5px] border-control-line bg-app shadow-[0_10px_30px_-18px_rgba(0,0,0,0.45)] ${radius}`}
        >
            {surface.chrome !== 'none' && (
                <div
                    className="flex shrink-0 items-center gap-1.5 border-b border-line bg-panel px-3 text-[11px] text-muted"
                    style={{ height: CHROME_HEIGHT }}
                >
                    {surface.chrome === 'window' && (
                        <>
                            <span className="h-2 w-2 rounded-full border border-control-line" />
                            <span className="h-2 w-2 rounded-full border border-control-line" />
                            <span className="h-2 w-2 rounded-full border border-control-line" />
                            <span className="ml-2 truncate">{screen.title}</span>
                        </>
                    )}
                    {surface.chrome === 'phone' && (
                        <>
                            <span className="font-semibold">9:41</span>
                            <span className="flex-1" />
                            <span className="h-1.5 w-8 rounded-full bg-control-line" />
                        </>
                    )}
                    {surface.chrome === 'dialog' && (
                        <span className="truncate font-semibold">{screen.title}</span>
                    )}
                </div>
            )}
            <div className="min-h-0 flex-1">
                <div className="wf-root" dangerouslySetInnerHTML={{ __html: html }} />
            </div>
        </div>
    );
}

export function StateChip({ state }: { state: NonNullable<VisualScreen['state']> }) {
    const tone = state === 'before' ? 'bg-del-bg text-del' : 'bg-add-bg text-add';

    return (
        <span
            className={`rounded-full px-2 py-0.5 text-[10.5px] font-semibold tracking-wide uppercase ${tone}`}
        >
            {STATE_LABELS[state]}
        </span>
    );
}

function BoardContent({ canvas, layout }: { canvas: VisualCanvas; layout: CanvasLayout }) {
    return (
        <>
            <svg
                aria-hidden
                className="pointer-events-none absolute top-0 left-0 overflow-visible"
                height={layout.height}
                width={layout.width}
            >
                <defs>
                    <marker
                        id="vz-step"
                        markerHeight="8"
                        markerWidth="8"
                        orient="auto-start-reverse"
                        refX="7"
                        refY="4"
                    >
                        <path d="M0,0 L8,4 L0,8 z" fill="var(--accent)" />
                    </marker>
                </defs>
                {layout.connectors.map((connector, index) => (
                    <path
                        d={connector.path}
                        fill="none"
                        key={index}
                        markerEnd="url(#vz-step)"
                        stroke="var(--accent)"
                        strokeDasharray="6 5"
                        strokeWidth="1.6"
                    />
                ))}
            </svg>
            {layout.connectors.map(
                (connector, index) =>
                    connector.label && (
                        <span
                            className="absolute max-w-[170px] -translate-x-1/2 -translate-y-1/2 rounded-full border border-accent-line bg-panel px-2 py-0.5 text-center text-[11.5px] leading-snug font-semibold text-accent-strong"
                            key={index}
                            style={{ left: connector.x, top: connector.y }}
                        >
                            {connector.label}
                        </span>
                    ),
            )}
            {layout.screens.map(({ height, screen, width, x, y }) => (
                <div key={screen.id}>
                    <div
                        className="absolute flex items-center gap-2 text-[13px] font-semibold whitespace-nowrap"
                        style={{ left: x, top: y - 28, width }}
                    >
                        {screen.state && <StateChip state={screen.state} />}
                        <span className="truncate">{screen.title || screen.id}</span>
                    </div>
                    <div className="absolute" style={{ height, left: x, top: y, width }}>
                        <ScreenFrame canvas={canvas} screen={screen} />
                    </div>
                    <ScreenNotes
                        canvas={canvas}
                        screen={screen}
                        width={width}
                        x={x}
                        y={y + height + 14}
                    />
                </div>
            ))}
        </>
    );
}

function CanvasBoard({
    canvas,
    framed,
    layout,
}: {
    canvas: VisualCanvas;
    framed: boolean;
    layout: CanvasLayout;
}) {
    return (
        <Board
            className={
                framed
                    ? 'h-[clamp(300px,46vh,560px)] rounded-xl border border-line'
                    : 'h-[clamp(320px,52vh,640px)] border-y border-line'
            }
            height={layout.height}
            label="Wireframes"
            width={layout.width}
        >
            <BoardContent canvas={canvas} layout={layout} />
        </Board>
    );
}

function ScreenNotes({
    canvas,
    screen,
    width,
    x,
    y,
}: {
    canvas: VisualCanvas;
    screen: VisualScreen;
    width: number;
    x: number;
    y: number;
}) {
    const pins = canvas.pins.filter((pin) => pin.screen === screen.id);

    if (!screen.note && !pins.length) {
        return null;
    }

    return (
        <div
            className="absolute flex flex-col gap-2.5 text-[13px]"
            style={{ left: x, top: y, width }}
        >
            {screen.note && <Inline className="leading-normal text-body" text={screen.note} />}
            {pins.map((pin) => (
                <div className="flex gap-2" key={`${pin.screen}-${pin.number}`}>
                    <span className="mt-px flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-add text-[11px] font-semibold text-on-accent">
                        {pin.number}
                    </span>
                    <span className="leading-normal">
                        <Inline className="font-semibold" text={pin.title} />
                        {pin.body && (
                            <>
                                <span className="text-muted"> — </span>
                                <Inline className="text-muted" text={pin.body} />
                            </>
                        )}
                    </span>
                </div>
            ))}
        </div>
    );
}
