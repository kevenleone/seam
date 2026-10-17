import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

import type { Visual, VisualKind, VisualState } from '../lib/types';

import { Icon } from '../components/Icon';
import { ProviderSelect } from '../components/ProviderSelect';
import { Button, relativeTime, TextArea } from '../components/ui';
import { VisualDocument } from '../components/visual/VisualDocument';
import { PROVIDER_LABELS } from '../lib/ai';
import { api, errorMessage } from '../lib/api';
import { useSettingsModal } from '../lib/settingsModal';
import { branchLabel, useRepo } from '../lib/store';
import { type ExportFormat, exportName, exportVisual, EXTENSIONS } from '../lib/visualExport';
import { Running } from './ReviewView';

const KINDS: { id: VisualKind; label: string }[] = [
    { id: 'recap', label: 'Recap' },
    { id: 'plan', label: 'Plan' },
];

const FORMATS: { hint: string; id: 'pdf' | ExportFormat; label: string }[] = [
    { hint: 'One file, opens in any browser', id: 'html', label: 'Web page' },
    { hint: 'Through the print panel', id: 'pdf', label: 'PDF…' },
    { hint: 'For a pull request or an agent', id: 'markdown', label: 'Markdown' },
    { hint: 'Editable screens and diagrams', id: 'excalidraw', label: 'Excalidraw' },
];

export function VisualView() {
    const kind = useRepo((state) => state.visualKind);
    const state = useRepo((state) => (state.visualKind === 'plan' ? state.plan : state.recap));
    const shown = shownVisual(state);
    const [composing, setComposing] = useState(false);
    const [printing, setPrinting] = useState<null | Visual>(null);

    useEffect(() => setComposing(false), [kind, shown?.id]);

    useEffect(() => {
        if (!printing) {
            return;
        }

        // Two frames: one to mount the print copy, one for it to lay out.
        const frame = requestAnimationFrame(() =>
            requestAnimationFrame(() => {
                api.printWindow().catch((error) =>
                    useRepo.getState().set({ error: errorMessage(error) }),
                );
            }),
        );

        return () => cancelAnimationFrame(frame);
    }, [printing]);

    const composer = kind === 'plan' && (composing || !shown) && state.status !== 'running';

    return (
        <section aria-label="Visual" className="flex min-w-0 flex-1 flex-col">
            <VisualHeader
                kind={kind}
                onCompose={() => setComposing(true)}
                onPrint={setPrinting}
                shown={shown}
                state={state}
            />
            <div className="min-h-0 flex-1 overflow-auto">
                {state.status === 'running' ? (
                    <Running
                        detail={state.kind === 'plan' ? 'exploring the repository' : state.scope}
                        expected="most take two to five minutes"
                        onCancel={() => useRepo.getState().cancelVisual(state.kind)}
                        startedAt={state.startedAt}
                        title={`${PROVIDER_LABELS[state.provider]} is drawing ${state.kind === 'plan' ? 'a plan' : `a recap of ${state.scope}`}`}
                    />
                ) : (
                    <>
                        {state.status === 'failed' && <Failure kind={kind} state={state} />}
                        {composer && (
                            <PlanComposer
                                initial={composing ? '' : (shown?.prompt ?? '')}
                                onCancel={shown ? () => setComposing(false) : undefined}
                            />
                        )}
                        {shown && !composer && <VisualDocument mode="app" visual={shown} />}
                        {!shown && kind === 'recap' && <RecapIntro />}
                    </>
                )}
            </div>
            {printing &&
                createPortal(
                    <div className="vz-print">
                        <VisualDocument mode="print" visual={printing} />
                    </div>,
                    document.body,
                )}
        </section>
    );
}

function ExportMenu({ onPrint, visual }: { onPrint(visual: Visual): void; visual: Visual }) {
    const [open, setOpen] = useState(false);
    const [busy, setBusy] = useState(false);
    const menu = useRef<HTMLDivElement>(null);

    useEffect(() => {
        if (!open) {
            return;
        }

        const close = (event: PointerEvent) =>
            !menu.current?.contains(event.target as Node) && setOpen(false);

        window.addEventListener('pointerdown', close);

        return () => window.removeEventListener('pointerdown', close);
    }, [open]);

    const run = async (format: 'pdf' | ExportFormat) => {
        setOpen(false);

        if (format === 'pdf') {
            onPrint(visual);

            return;
        }

        setBusy(true);

        try {
            const contents = await exportVisual(visual, format);
            const saved = await api.exportVisual(exportName(visual), EXTENSIONS[format], contents);

            if (saved) {
                useRepo.getState().notify('Exported');
            }
        } catch (error) {
            useRepo.getState().set({ error: errorMessage(error) });
        } finally {
            setBusy(false);
        }
    };

    return (
        <div className="relative" ref={menu}>
            <Button
                aria-expanded={open}
                aria-haspopup="menu"
                disabled={busy}
                onClick={() => setOpen(!open)}
                size="sm"
            >
                <Icon name="download" size={12} /> {busy ? 'Exporting…' : 'Export'}
            </Button>
            {open && (
                <div
                    className="absolute top-full right-0 z-20 mt-1 flex w-[250px] flex-col rounded-lg border border-control-line bg-panel p-1 shadow-[0_10px_30px_var(--shadow)]"
                    role="menu"
                >
                    {FORMATS.map((format) => (
                        <button
                            className="flex flex-col rounded-md px-2.5 py-1.5 text-left hover:bg-hover"
                            key={format.id}
                            onClick={() => void run(format.id)}
                            role="menuitem"
                            type="button"
                        >
                            <span>{format.label}</span>
                            <span className="text-[11.5px] text-muted">{format.hint}</span>
                        </button>
                    ))}
                </div>
            )}
        </div>
    );
}

function Failure({
    kind,
    state,
}: {
    kind: VisualKind;
    state: Extract<VisualState, { status: 'failed' }>;
}) {
    const { startVisual } = useRepo();

    return (
        <div
            className="flex items-start gap-2.5 border-b border-del-line bg-del-bg px-[18px] py-2.5"
            role="alert"
        >
            <Icon className="mt-0.5 shrink-0 text-del" name="warning" size={14} />
            <pre className="selectable m-0 flex-1 font-sans whitespace-pre-wrap text-body">
                {state.message}
            </pre>
            <Button
                onClick={() => useSettingsModal.getState().show('ai')}
                size="sm"
                variant="ghost"
            >
                AI settings
            </Button>
            {kind === 'recap' && (
                <Button onClick={() => startVisual('recap', undefined, state.provider)} size="sm">
                    Try again
                </Button>
            )}
        </div>
    );
}

function PlanComposer({ initial, onCancel }: { initial: string; onCancel?: () => void }) {
    const { startVisual } = useRepo();
    const [prompt, setPrompt] = useState(initial);

    const submit = () => prompt.trim() && startVisual('plan', prompt.trim());

    return (
        <div className="mx-auto flex w-full max-w-[720px] flex-col gap-3 px-8 py-10">
            <h2 className="m-0 text-[18px] font-semibold">Plan a change</h2>
            <p className="m-0 leading-normal text-muted">
                Describe what you want built. The model explores the repository read-only and draws
                a plan to approve before any code is written: the screens before and after, the
                architecture, the decisions, the files it would touch and the questions left open.
            </p>
            <TextArea
                aria-label="The change to plan"
                autoFocus
                className="min-h-[140px]"
                onChange={(event) => setPrompt(event.target.value)}
                onKeyDown={(event) => {
                    if (event.key === 'Enter' && event.metaKey) {
                        event.preventDefault();
                        submit();
                    }
                }}
                placeholder="Add a Visual tab that draws a recap of the branch…"
                value={prompt}
            />
            <div className="flex items-center gap-2">
                <Button disabled={!prompt.trim()} onClick={submit} variant="primary">
                    <Icon name="sparkle" size={12} /> Draw plan
                </Button>
                {onCancel && <Button onClick={onCancel}>Cancel</Button>}
                <span className="text-[12px] text-dim">⌘↵</span>
            </div>
        </div>
    );
}

function RecapIntro() {
    const { startVisual, status } = useRepo();

    return (
        <div className="flex h-full flex-col items-center justify-center gap-3 p-8 text-center">
            <Icon className="text-accent" name="sparkle" size={30} />
            <h2 className="m-0 text-[16px] font-semibold">
                See what {branchLabel(status)} changes
            </h2>
            <p className="m-0 max-w-md leading-normal text-muted">
                The model reads the branch against its base and draws a map of it: the screens it
                touches before and after, how the pieces fit, the decisions behind it, the diffs
                worth reading and how to test it. Export it as a web page, PDF, Markdown or
                Excalidraw.
            </p>
            <Button onClick={() => startVisual('recap')} variant="primary">
                Recap {branchLabel(status)}
            </Button>
        </div>
    );
}

function shownVisual(state: VisualState): null | Visual {
    if (state.status === 'done') {
        return state.visual;
    }

    return state.status === 'failed' ? state.previous : null;
}

function VisualHeader({
    kind,
    onCompose,
    onPrint,
    shown,
    state,
}: {
    kind: VisualKind;
    onCompose(): void;
    onPrint(visual: Visual): void;
    shown: null | Visual;
    state: VisualState;
}) {
    const { plans, set, startVisual, status } = useRepo();
    const running = state.status === 'running';
    const stale =
        kind === 'recap' &&
        shown?.headSha &&
        status.branch.oid &&
        shown.headSha !== status.branch.oid;

    return (
        <div className="flex shrink-0 flex-wrap items-center gap-2.5 border-b border-line px-[18px] py-2.5">
            <nav
                aria-label="Kind"
                className="flex gap-0.5 rounded-[7px] border border-control-line bg-control p-0.5"
            >
                {KINDS.map((option) => (
                    <button
                        aria-current={kind === option.id ? 'page' : undefined}
                        className={`rounded-[5px] px-2.5 py-0.5 text-[12.5px] ${kind === option.id ? 'bg-tab text-fg' : 'text-muted hover:text-fg'}`}
                        key={option.id}
                        onClick={() => set({ visualKind: option.id })}
                        type="button"
                    >
                        {option.label}
                    </button>
                ))}
            </nav>
            {shown && (
                <span className="truncate text-[12px] text-muted">
                    {kind === 'recap' ? shown.scope : 'Planned'} · {relativeTime(shown.createdAt)}{' '}
                    by {PROVIDER_LABELS[shown.provider]}
                </span>
            )}
            {stale && (
                <span className="rounded-full bg-warn-bg px-2 py-0.5 text-[11.5px] text-warn">
                    New commits since this recap
                </span>
            )}
            <span className="flex-1" />
            {kind === 'plan' && plans.length > 0 && (
                <select
                    aria-label="Earlier plans"
                    className="h-[26px] max-w-[200px] rounded-md border border-control-line bg-control px-1.5 text-[12px] text-fg"
                    disabled={running}
                    onChange={(event) =>
                        event.target.value && useRepo.getState().openPlan(event.target.value)
                    }
                    value={shown?.id ?? ''}
                >
                    <option disabled value="">
                        Earlier plans
                    </option>
                    {plans.map((plan) => (
                        <option key={plan.id} value={plan.id}>
                            {plan.title}
                        </option>
                    ))}
                </select>
            )}
            <ProviderSelect disabled={running} />
            <Button
                aria-label="AI settings"
                onClick={() => useSettingsModal.getState().show('ai')}
                size="sm"
                title="AI settings"
                variant="ghost"
            >
                <Icon name="gear" size={13} />
            </Button>
            {shown && <ExportMenu onPrint={onPrint} visual={shown} />}
            {kind === 'recap' ? (
                <Button
                    disabled={running}
                    onClick={() => startVisual('recap')}
                    size="sm"
                    title="Recap the branch (⇧⌘V)"
                    variant={shown ? 'default' : 'primary'}
                >
                    <Icon name="refresh" size={12} />
                    {shown ? 'Redraw recap' : `Recap ${branchLabel(status)}`}
                </Button>
            ) : (
                shown && (
                    <Button disabled={running} onClick={onCompose} size="sm">
                        <Icon name="plus" size={12} /> New plan
                    </Button>
                )
            )}
        </div>
    );
}
