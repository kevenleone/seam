import {
    type CSSProperties,
    type ReactNode,
    useEffect,
    useLayoutEffect,
    useRef,
    useState,
} from 'react';
import { createPortal } from 'react-dom';

import { Icon } from '../Icon';

const MIN_ZOOM = 0.1;
const MAX_ZOOM = 2;

/** A pannable, zoomable surface for content laid out at a fixed size, fitted until the reader moves it. */
export function Board({
    children,
    className,
    height,
    label,
    style,
    width,
}: {
    children: ReactNode;
    className: string;
    height: number;
    label: string;
    style?: CSSProperties;
    width: number;
}) {
    const viewport = useRef<HTMLDivElement>(null);
    const [view, setView] = useState({ x: 0, y: 0, zoom: 1 });
    const [expanded, setExpanded] = useState(false);
    const drag = useRef<{ x: number; y: number } | null>(null);
    const moved = useRef(false);

    const fit = () => {
        const element = viewport.current;

        if (!element) {
            return;
        }

        moved.current = false;

        const zoom = Math.max(
            MIN_ZOOM,
            Math.min(1, element.clientWidth / width, element.clientHeight / height),
        );

        setView({
            x: (element.clientWidth - width * zoom) / 2,
            y: Math.max(0, (element.clientHeight - height * zoom) / 2),
            zoom,
        });
    };

    useLayoutEffect(fit, [width, height, expanded]);

    // Until the reader moves the board, it stays fitted as the window and panels change size.
    useEffect(() => {
        const element = viewport.current;

        if (!element) {
            return;
        }

        const observer = new ResizeObserver(() => !moved.current && fit());

        observer.observe(element);

        return () => observer.disconnect();
    });

    useEffect(() => {
        const element = viewport.current;

        if (!element) {
            return;
        }

        // A non-passive listener, so a pinch or scroll moves the board instead of the page.
        const onWheel = (event: WheelEvent) => {
            event.preventDefault();
            moved.current = true;

            const bounds = element.getBoundingClientRect();
            const pointX = event.clientX - bounds.left;
            const pointY = event.clientY - bounds.top;

            setView((current) => {
                if (!event.ctrlKey && !event.metaKey) {
                    return { ...current, x: current.x - event.deltaX, y: current.y - event.deltaY };
                }

                const zoom = Math.min(
                    MAX_ZOOM,
                    Math.max(MIN_ZOOM, current.zoom * Math.exp(-event.deltaY / 200)),
                );
                const ratio = zoom / current.zoom;

                return {
                    x: pointX - (pointX - current.x) * ratio,
                    y: pointY - (pointY - current.y) * ratio,
                    zoom,
                };
            });
        };

        element.addEventListener('wheel', onWheel, { passive: false });

        return () => element.removeEventListener('wheel', onWheel);
    }, [expanded]);

    useEffect(() => {
        if (!expanded) {
            return;
        }

        const onKey = (event: KeyboardEvent) => {
            if (event.key === 'Escape') {
                event.stopPropagation();
                setExpanded(false);
            }
        };

        window.addEventListener('keydown', onKey, true);

        return () => window.removeEventListener('keydown', onKey, true);
    }, [expanded]);

    const zoomBy = (factor: number) => {
        const element = viewport.current;

        if (!element) {
            return;
        }

        moved.current = true;
        setView((current) => {
            const zoom = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, current.zoom * factor));
            const centerX = element.clientWidth / 2;
            const centerY = element.clientHeight / 2;
            const ratio = zoom / current.zoom;

            return {
                x: centerX - (centerX - current.x) * ratio,
                y: centerY - (centerY - current.y) * ratio,
                zoom,
            };
        });
    };

    const board = (
        <div
            aria-label={label}
            className={`relative cursor-grab overflow-hidden bg-canvas bg-[radial-gradient(var(--line)_1px,transparent_1px)] bg-size-[18px_18px] active:cursor-grabbing ${expanded ? 'h-full rounded-xl border border-line shadow-2xl' : className}`}
            onDoubleClick={(event) => !(event.target as HTMLElement).closest('button') && fit()}
            onPointerDown={(event) => {
                if ((event.target as HTMLElement).closest('button')) {
                    return;
                }

                event.currentTarget.setPointerCapture(event.pointerId);
                drag.current = { x: event.clientX, y: event.clientY };
            }}
            onPointerMove={(event) => {
                const start = drag.current;

                if (!start) {
                    return;
                }

                drag.current = { x: event.clientX, y: event.clientY };
                moved.current = true;
                setView((current) => ({
                    ...current,
                    x: current.x + event.clientX - start.x,
                    y: current.y + event.clientY - start.y,
                }));
            }}
            onPointerUp={() => (drag.current = null)}
            ref={viewport}
            role="region"
            style={expanded ? undefined : style}
        >
            <div
                className="absolute top-0 left-0 origin-top-left"
                style={{
                    height,
                    transform: `translate(${view.x}px, ${view.y}px) scale(${view.zoom})`,
                    width,
                }}
            >
                {children}
            </div>
            <div className="absolute bottom-3 left-3 flex items-center gap-0.5 rounded-lg border border-line bg-panel p-0.5 text-[12px] shadow-sm">
                <button
                    aria-label="Zoom out"
                    className="h-6 w-6 rounded-md hover:bg-hover"
                    onClick={() => zoomBy(1 / 1.25)}
                    type="button"
                >
                    −
                </button>
                <span className="w-11 text-center font-mono" title="Zoom">
                    {Math.round(view.zoom * 100)}%
                </span>
                <button
                    aria-label="Zoom in"
                    className="h-6 w-6 rounded-md hover:bg-hover"
                    onClick={() => zoomBy(1.25)}
                    type="button"
                >
                    +
                </button>
                <button
                    className="h-6 rounded-md px-2 text-muted hover:bg-hover hover:text-fg"
                    onClick={fit}
                    title="Fit everything in view (double-click)"
                    type="button"
                >
                    Fit
                </button>
                <button
                    aria-label={expanded ? 'Close (esc)' : 'Fill the window'}
                    className="flex h-6 w-6 items-center justify-center rounded-md text-muted hover:bg-hover hover:text-fg"
                    onClick={() => setExpanded(!expanded)}
                    title={expanded ? 'Close (esc)' : 'Fill the window'}
                    type="button"
                >
                    <Icon name={expanded ? 'collapse' : 'expand'} size={12} />
                </button>
            </div>
        </div>
    );

    if (!expanded) {
        return board;
    }

    return (
        <>
            <div className={className} style={style} />
            {createPortal(
                <div
                    className="fixed inset-0 z-40 bg-black/40 p-6"
                    onMouseDown={(event) =>
                        event.target === event.currentTarget && setExpanded(false)
                    }
                >
                    {board}
                </div>,
                document.body,
            )}
        </>
    );
}
