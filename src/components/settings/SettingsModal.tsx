import { getCurrentWindow } from '@tauri-apps/api/window';
import { useEffect, useMemo, useRef, useState } from 'react';

import type { SettingsPane } from '../../lib/settingsModal';

import { restoreDefaults } from '../../lib/settings';
import { useSettingsModal } from '../../lib/settingsModal';
import { useAppName, useAppVersion } from '../../lib/version';
import { Icon, type IconName, SeamMark } from '../Icon';
import { AboutPane, AiPane, AppearancePane, GeneralPane, ShortcutsPane } from './panes';

interface PaneDefinition {
    Body: () => React.JSX.Element;
    icon: IconName;
    id: SettingsPane;
    label: string;
    /** What the pane holds, so searching for a setting by name finds its pane. */
    terms: string;
}

const PANES: PaneDefinition[] = [
    {
        Body: GeneralPane,
        icon: 'gear',
        id: 'general',
        label: 'General',
        terms: 'git executable path pull fast-forward rebase merge diff layout unified split whitespace syntax highlighting colours code languages worktree window replace close menu bar tray recent repositories terminal command line cli shell path install notification notify sound alert finished',
    },
    {
        Body: AppearancePane,
        icon: 'palette',
        id: 'appearance',
        label: 'Appearance',
        terms: 'colour color mode light dark auto theme style accent code size font status bar motion animation',
    },
    {
        Body: AiPane,
        icon: 'sparkle',
        id: 'ai',
        label: 'AI',
        terms: 'ai review model agent claude code codex opencode anthropic openai gemini google ollama lm studio openrouter endpoint api key test connection picky language bugs security tests breaking performance style',
    },
    {
        Body: ShortcutsPane,
        icon: 'keyboard',
        id: 'shortcuts',
        label: 'Keyboard Shortcuts',
        terms: 'keys hotkeys commit stage undo sidebar panel expand review visual recap',
    },
    {
        Body: AboutPane,
        icon: 'info',
        id: 'about',
        label: 'About',
        terms: 'version build licence license source github release notes issue telemetry account',
    },
];

/** Settings as a sheet over a repository window, or (`standalone`) filling a window of their own. */
export function SettingsModal({ standalone }: { standalone?: boolean }) {
    const modal = useSettingsModal();
    const { pane, show } = modal;
    const open = standalone || modal.open;
    const close = standalone ? () => void getCurrentWindow().close() : modal.close;
    const [filter, setFilter] = useState('');
    const sheet = useRef<HTMLDivElement>(null);
    const name = useAppName();
    const version = useAppVersion();

    useEffect(() => {
        if (!open) {
            return;
        }

        const onKey = (event: KeyboardEvent) => {
            if (event.key === 'Escape') {
                event.stopPropagation();
                close();
            }
        };

        window.addEventListener('keydown', onKey, true);
        sheet.current?.focus();

        return () => window.removeEventListener('keydown', onKey, true);
    }, [open, close]);

    const visible = useMemo(() => {
        const query = filter.trim().toLowerCase();

        return query
            ? PANES.filter(
                  (definition) =>
                      definition.label.toLowerCase().includes(query) ||
                      definition.terms.includes(query),
              )
            : PANES;
    }, [filter]);

    if (!open) {
        return null;
    }

    const active = PANES.find((definition) => definition.id === pane) ?? PANES[0];

    return (
        <div
            className={
                standalone ? 'flex h-full' : 'fixed inset-0 z-50 flex items-center justify-center'
            }
        >
            {!standalone && (
                <div className="absolute inset-0 bg-black/45 backdrop-blur-[2px]" onClick={close} />
            )}
            <div
                aria-label="Seam settings"
                aria-modal={standalone ? undefined : 'true'}
                className={
                    standalone
                        ? 'flex h-full w-full overflow-hidden bg-app outline-none'
                        : 'relative flex h-[min(700px,calc(100%-64px))] w-[min(1000px,calc(100%-64px))] overflow-hidden rounded-2xl border border-control-line bg-app shadow-[0_40px_90px_-24px_var(--shadow)] outline-none'
                }
                ref={sheet}
                role={standalone ? undefined : 'dialog'}
                tabIndex={-1}
            >
                <nav
                    aria-label="Settings sections"
                    className={`flex w-[232px] flex-none flex-col border-r border-line bg-panel px-2.5 pb-2.5 ${standalone ? 'pt-[52px]' : 'pt-3.5'}`}
                    data-tauri-drag-region={standalone || undefined}
                >
                    <label className="mb-3 flex items-center gap-2 rounded-lg bg-control px-2.5 py-1.5 text-muted">
                        <Icon name="search" size={13} />
                        <input
                            className="min-w-0 flex-1 border-none bg-transparent text-fg outline-none"
                            onChange={(event) => setFilter(event.target.value)}
                            placeholder="Search settings"
                            value={filter}
                        />
                    </label>
                    <div className="flex flex-col gap-px overflow-auto">
                        {visible.map((definition) => {
                            const on = definition.id === active.id;

                            return (
                                <button
                                    aria-current={on ? 'page' : undefined}
                                    className={`flex items-center gap-2.5 rounded-lg px-2 py-1.5 text-left ${on ? 'bg-accent-bg font-semibold text-fg' : 'text-soft hover:bg-hover'}`}
                                    key={definition.id}
                                    onClick={() => show(definition.id)}
                                    type="button"
                                >
                                    <span
                                        className={`flex h-6 w-6 flex-none items-center justify-center rounded-md ${on ? 'bg-accent text-on-accent' : 'bg-control text-soft'}`}
                                    >
                                        <Icon name={definition.icon} size={14} />
                                    </span>
                                    {definition.label}
                                </button>
                            );
                        })}
                        {visible.length === 0 && (
                            <span className="px-2 py-2 text-muted">No matching settings.</span>
                        )}
                    </div>
                    <span className="flex-1" />
                    <div className="flex flex-col gap-1.5 border-t border-line px-2.5 pt-2.5">
                        <button
                            className="text-left text-[12px] text-muted hover:text-fg"
                            onClick={restoreDefaults}
                            type="button"
                        >
                            Restore defaults
                        </button>
                        <span className="flex items-center gap-2 text-[12px] text-dim">
                            <SeamMark size={13} /> {name} {version}
                        </span>
                    </div>
                </nav>
                <section className="flex min-w-0 flex-1 flex-col">
                    <header
                        className="flex h-[52px] flex-none items-center border-b border-line pr-4 pl-6"
                        data-tauri-drag-region={standalone || undefined}
                    >
                        <h2 className="m-0 text-[15px] font-semibold">{active.label}</h2>
                        <button
                            aria-label="Close settings"
                            className="ml-auto rounded-lg p-1.5 text-muted hover:bg-hover hover:text-fg"
                            onClick={close}
                            type="button"
                        >
                            <Icon name="x" size={16} />
                        </button>
                    </header>
                    <div className="flex-1 overflow-auto px-6 pt-6 pb-8">
                        <active.Body />
                    </div>
                </section>
            </div>
        </div>
    );
}
