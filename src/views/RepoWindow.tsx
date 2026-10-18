import { listen } from '@tauri-apps/api/event';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { lazy, Suspense, useEffect, useState } from 'react';

import { Icon } from '../components/Icon';
import { Palette } from '../components/Palette';
import { SettingsModal } from '../components/settings/SettingsModal';
import { Sheets } from '../components/Sheets';
import { Button } from '../components/ui';
import { api, errorMessage } from '../lib/api';
import { getSettings, syncRecents, type Theme, updateSettings, useSettings } from '../lib/settings';
import { useSettingsModal } from '../lib/settingsModal';
import { branchLabel, changeCount, repoName, type Tab, useRepo } from '../lib/store';
import { useAppName, useAppVersion } from '../lib/version';
import { ChangesView } from './ChangesView';
import { HistoryView } from './HistoryView';
import { ReviewView } from './ReviewView';

// Its diagrams, markdown and sanitiser load the first time the tab opens.
const VisualView = lazy(() =>
    import('./VisualView').then((module) => ({ default: module.VisualView })),
);

export function RepoWindow({ root }: { root: string }) {
    const tab = useRepo((state) => state.tab);
    const loaded = useRepo((state) => state.loaded);
    const palette = useRepo((state) => state.palette);
    const expanded = useRepo((state) => state.expanded);
    const { statusBar } = useSettings();

    useEffect(() => {
        void syncRecents();
        void getCurrentWebviewWindow().setTitle(repoName(root));

        // The tray opens a window straight onto its review with ?tab=review.
        const initialTab = new URLSearchParams(location.search).get('tab');

        if (initialTab === 'review' || initialTab === 'history' || initialTab === 'visual') {
            useRepo.getState().set({ tab: initialTab });
        }

        void useRepo.getState().init(root);
        void api.watch(root);

        const win = getCurrentWebviewWindow();
        const unlisten = [
            win.listen<{ refs: boolean; workTree: boolean }>('repo-changed', (event) => {
                const s = useRepo.getState();

                if (s.busy) {
                    return;
                }

                void s.refreshStatus();

                if (event.payload.refs) {
                    void s.refreshHistory();
                }
            }),
            win.listen<string>('git-progress', (event) =>
                useRepo.getState().set({ progress: event.payload }),
            ),
            win.listen<string>('menu', (event) => onMenu(event.payload)),
            win.listen<Tab>('show-tab', (event) => useRepo.getState().set({ tab: event.payload })),
            listen<string>('review-updated', (event) => {
                if (event.payload === root) {
                    void useRepo.getState().loadReview();
                }
            }),
            listen<string>('visual-updated', (event) => {
                if (event.payload === root) {
                    void useRepo.getState().loadVisuals();
                }
            }),
            listen('tauri://focus', () => void useRepo.getState().refreshStatus()),
        ];

        const onKey = (event: KeyboardEvent) => {
            const s = useRepo.getState();
            const modal = useSettingsModal.getState().open;

            if (event.key === 'Escape' && s.expanded && !s.sheet && !s.palette && !modal) {
                s.set({ expanded: false });

                return;
            }

            if (!event.metaKey || modal) {
                return;
            }

            // ⌘Z in a text field undoes typing; elsewhere it undoes the last git action.
            if (
                event.key.toLowerCase() === 'z' &&
                !isEditable(event.target) &&
                !s.sheet &&
                !s.palette
            ) {
                event.preventDefault();

                if (event.shiftKey) {
                    s.redo();
                } else {
                    s.undo();
                }
            }

            if (event.key === 'Enter' && s.tab === 'changes' && !s.sheet && !s.palette) {
                event.preventDefault();
                s.commit();
            }
        };

        window.addEventListener('keydown', onKey);

        return () => {
            unlisten.forEach((promise) => promise.then((stop) => stop()));
            window.removeEventListener('keydown', onKey);
        };
    }, [root]);

    if (!loaded) {
        return (
            <div
                className="flex h-full items-center justify-center bg-app text-muted"
                data-tauri-drag-region
            >
                Opening {repoName(root)}…
            </div>
        );
    }

    return (
        <div className="flex h-full flex-col bg-app text-fg">
            <Header />
            <Banners />
            <main className="flex min-h-0 flex-1">
                {tab === 'changes' ? (
                    <ChangesView />
                ) : tab === 'history' ? (
                    <HistoryView />
                ) : tab === 'review' ? (
                    <ReviewView />
                ) : (
                    <Suspense fallback={null}>
                        <VisualView />
                    </Suspense>
                )}
            </main>
            {statusBar && !expanded && <StatusBar />}
            <ToastView />
            {palette && <Palette />}
            <Sheets />
            <SettingsModal />
        </div>
    );
}

function Banners() {
    const { error, inProgress, perform, set, status } = useRepo();

    return (
        <>
            {inProgress && (
                <div
                    className="flex flex-wrap items-center gap-3 border-b border-del-line bg-warn-bg px-[18px] py-2.5"
                    role="status"
                >
                    <span className="font-semibold text-warn">
                        {inProgress.kind === 'rebase'
                            ? `Rebasing${inProgress.headName ? ` ${inProgress.headName}` : ''}`
                            : inProgress.kind === 'merge'
                              ? 'Merging'
                              : inProgress.kind === 'cherryPick'
                                ? 'Cherry-picking'
                                : 'Reverting'}
                    </span>
                    <span className="text-body">
                        {inProgress.kind === 'rebase' && inProgress.step && inProgress.total
                            ? `Commit ${inProgress.step} of ${inProgress.total} · `
                            : ''}
                        {status.conflicted.length
                            ? 'Resolve the conflicted files in Changes, then continue.'
                            : 'No conflicts left. Continue when you’re ready.'}
                    </span>
                    <span className="flex-1" />
                    <Button onClick={() => perform({ type: 'abort' }, 'Aborting')} variant="danger">
                        Abort
                    </Button>
                    {inProgress.kind === 'rebase' && (
                        <Button
                            onClick={() => perform({ type: 'skip' }, 'Skipping')}
                            variant="danger"
                        >
                            Skip this commit
                        </Button>
                    )}
                    <Button
                        disabled={status.conflicted.length > 0}
                        onClick={() => perform({ type: 'continue' }, 'Continuing')}
                        variant="primary"
                    >
                        {status.conflicted.length
                            ? `Continue · ${status.conflicted.length} conflict${status.conflicted.length === 1 ? '' : 's'} left`
                            : 'Continue'}
                    </Button>
                </div>
            )}
            {error && (
                <div
                    className="flex items-start gap-2.5 border-b border-del-line bg-del-bg px-[18px] py-2.5"
                    role="alert"
                >
                    <Icon className="mt-0.5 shrink-0 text-del" name="warning" size={14} />
                    <pre className="selectable m-0 flex-1 font-sans whitespace-pre-wrap text-body">
                        {error}
                    </pre>
                    <button
                        aria-label="Dismiss"
                        className="text-muted hover:text-fg"
                        onClick={() => set({ error: null })}
                        type="button"
                    >
                        <Icon name="x" size={14} />
                    </button>
                </div>
            )}
        </>
    );
}

function Header() {
    const s = useRepo();
    const count = changeCount(s.status);
    const { ahead, behind, upstream } = s.status.branch;
    const [branchMenu, setBranchMenu] = useState(false);
    const local = s.meta.branches.filter((branch) => !branch.isRemote);

    return (
        <header
            className="flex min-h-[46px] shrink-0 items-center gap-3.5 border-b border-line bg-panel py-1.5 pr-3.5 pl-[84px]"
            data-tauri-drag-region
        >
            <PanelToggle side="left" />
            <span className="font-semibold" data-tauri-drag-region>
                {repoName(s.root)}
            </span>
            <div className="relative">
                <button
                    aria-expanded={branchMenu}
                    aria-haspopup="menu"
                    className="flex h-7 items-center gap-1.5 rounded-md border border-control-line bg-control px-2.5 font-mono text-[12px]"
                    onClick={() => setBranchMenu((previous) => !previous)}
                    type="button"
                >
                    <Icon className="text-muted" name="branch" size={12} />
                    {branchLabel(s.status)}
                    <Icon className="text-muted" name="chevronDown" size={11} />
                </button>
                {branchMenu && (
                    <>
                        <div className="fixed inset-0 z-30" onClick={() => setBranchMenu(false)} />
                        <div
                            className="absolute top-8 left-0 z-40 max-h-[60vh] w-72 overflow-auto rounded-lg border border-control-line bg-panel p-1 shadow-[0_12px_40px_var(--shadow)]"
                            role="menu"
                        >
                            <div className="px-2.5 pt-1.5 pb-1 text-[11px] font-semibold tracking-[0.08em] text-muted uppercase">
                                Switch to
                            </div>
                            {local.map((branch) =>
                                branch.worktree && !branch.isCurrent ? (
                                    <WorktreeItem
                                        branch={branch.name}
                                        key={branch.fullName}
                                        onOpen={(tab) => {
                                            setBranchMenu(false);
                                            api.openRepository(branch.worktree!, tab).catch(
                                                (error: unknown) => s.notify(errorMessage(error)),
                                            );
                                        }}
                                        path={branch.worktree}
                                    />
                                ) : (
                                    <button
                                        className="flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left font-mono text-[12px] hover:bg-hover disabled:text-accent"
                                        disabled={branch.isCurrent}
                                        key={branch.fullName}
                                        onClick={() => {
                                            setBranchMenu(false);
                                            s.perform(
                                                { branch: branch.name, type: 'checkout' },
                                                'Switching branch',
                                            );
                                        }}
                                        role="menuitem"
                                        type="button"
                                    >
                                        <span className="w-3">
                                            {branch.isCurrent && <Icon name="check" size={12} />}
                                        </span>
                                        <span className="flex-1 truncate">{branch.name}</span>
                                    </button>
                                ),
                            )}
                            <div className="my-1 border-t border-line" />
                            <button
                                className="flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left hover:bg-hover"
                                onClick={() => {
                                    setBranchMenu(false);
                                    s.set({ sheet: { from: null, kind: 'newBranch' } });
                                }}
                                role="menuitem"
                                type="button"
                            >
                                <Icon name="plus" size={12} /> New branch…
                            </button>
                        </div>
                    </>
                )}
            </div>
            <nav
                aria-label="Views"
                className="flex gap-0.5 rounded-[7px] border border-control-line bg-control p-0.5"
            >
                {(['changes', 'history', 'review', 'visual'] as const).map((option) => (
                    <button
                        aria-current={s.tab === option ? 'page' : undefined}
                        className={`rounded-[5px] px-2.5 py-1 ${s.tab === option ? 'bg-tab text-fg' : 'text-muted hover:text-fg'}`}
                        key={option}
                        onClick={() => s.set({ tab: option })}
                        type="button"
                    >
                        {
                            {
                                changes: 'Changes',
                                history: 'History',
                                review: 'Review',
                                visual: 'Visual',
                            }[option]
                        }
                        {option === 'changes' && count > 0 && (
                            <span className="ml-1.5 text-accent">{count}</span>
                        )}
                        {option === 'review' && <ReviewBadge />}
                        {option === 'visual' && <VisualBadge />}
                    </button>
                ))}
            </nav>
            <div className="flex-1" data-tauri-drag-region />
            <Button
                aria-label="Fetch"
                onClick={s.fetch}
                size="sm"
                title="Fetch from all remotes (⇧⌘F)"
                variant="ghost"
            >
                <Icon name="refresh" size={13} />
            </Button>
            <Button onClick={s.pull} size="sm" title="Pull (⇧⌘P)">
                <Icon name="down" size={12} /> Pull
                {behind > 0 && <span className="text-accent">↓{behind}</span>}
            </Button>
            <Button
                onClick={() => s.push()}
                size="sm"
                title={upstream ? `Push to ${upstream} (⌘P)` : 'Publish this branch (⌘P)'}
            >
                <Icon name="up" size={12} /> {upstream ? 'Push' : 'Publish'}
                {ahead > 0 && <span className="text-accent">↑{ahead}</span>}
            </Button>
            <Button
                aria-label="Command palette"
                onClick={() => s.set({ palette: true })}
                size="sm"
                title="Search and run any command (⌘K)"
                variant="ghost"
            >
                <Icon name="search" size={13} /> <span className="font-mono text-[11px]">⌘K</span>
            </Button>
        </header>
    );
}

function isEditable(target: EventTarget | null) {
    const el = target as HTMLElement | null;

    return !!el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.isContentEditable);
}

function onMenu(id: string) {
    const s = useRepo.getState();

    switch (id) {
        case 'expand-diff':
            return s.set({ expanded: !s.expanded });
        case 'fetch':
            return s.fetch();
        case 'new-branch':
            return s.set({ sheet: { from: null, kind: 'newBranch' } });
        case 'palette':
            return s.set({ palette: true });
        case 'plan-change':
            return s.set({ tab: 'visual', visualKind: 'plan' });
        case 'pull':
            return s.pull();
        case 'push':
            return s.push();
        case 'recap-branch':
            return s.startVisual('recap');
        case 'reveal':
            return void revealItemInDir(s.root);
        case 'review-branch':
            return s.startReview();
        case 'settings':
            return useSettingsModal.getState().show();
        case 'stage-all':
            return s.perform({ type: 'stageAll' }, 'Staging');
        case 'stash':
            return s.set({ sheet: { kind: 'stash', paths: [] } });
        case 'toggle-left':
            return updateSettings({ leftPanel: !getSettings().leftPanel });
        case 'toggle-right':
            return updateSettings({ rightPanel: !getSettings().rightPanel });
        case 'uncommit':
            return s.lastCommit && s.perform({ type: 'uncommit' }, 'Undoing commit');
        case 'unstage-all':
            return s.perform({ type: 'unstageAll' }, 'Unstaging');
        case 'view-changes':
            return s.set({ tab: 'changes' });
        case 'view-history':
            return s.set({ tab: 'history' });
        case 'view-review':
            return s.set({ tab: 'review' });
        case 'view-visual':
            return s.set({ tab: 'visual' });
    }
}

function PanelToggle({ side }: { side: 'left' | 'right' }) {
    const settings = useSettings();
    const open = side === 'left' ? settings.leftPanel : settings.rightPanel;
    const label = side === 'left' ? 'sidebar (⌘B)' : 'commit panel (⌘L)';

    return (
        <Button
            aria-label={`${open ? 'Hide' : 'Show'} ${label}`}
            aria-pressed={open}
            className={open ? 'text-fg' : ''}
            onClick={() =>
                updateSettings(side === 'left' ? { leftPanel: !open } : { rightPanel: !open })
            }
            size="sm"
            title={`${open ? 'Hide' : 'Show'} ${label}`}
            variant="ghost"
        >
            <Icon name={side === 'left' ? 'sidebarLeft' : 'sidebarRight'} size={14} />
        </Button>
    );
}

/** A pulsing dot while a review runs, then how many findings are still open. */
function ReviewBadge() {
    const review = useRepo((state) => state.review);
    const dismissed = useRepo((state) => state.dismissed);

    if (review.status === 'running') {
        return (
            <span
                aria-label="Review running"
                className="ml-1.5 inline-block h-1.5 w-1.5 animate-pulse rounded-full bg-accent align-[2px]"
            />
        );
    }

    if (review.status !== 'done') {
        return null;
    }

    const open = review.review.findings.filter((finding) => !dismissed.includes(finding.id)).length;

    return open > 0 ? <span className="ml-1.5 text-accent">{open}</span> : null;
}

function StatusBar() {
    const name = useAppName();
    const version = useAppVersion();
    const { busy, progress, root, status } = useRepo();
    const { theme } = useSettings();
    const { ahead, behind, head, upstream } = status.branch;

    return (
        <footer className="flex h-8 shrink-0 items-center gap-3.5 border-t border-line bg-panel px-3 text-[12px] text-muted">
            <span className="flex items-center gap-1.5">
                <Icon name="folder" size={12} />
                {repoName(root)}
            </span>
            <span className="flex items-center gap-1.5 font-mono text-[11.5px]">
                <Icon name="branch" size={12} />
                {branchLabel(status)}
            </span>
            {upstream ? (
                <span className="font-mono text-[11.5px]" title={`Ahead and behind ${upstream}`}>
                    ↑{ahead} ↓{behind}
                </span>
            ) : head ? (
                <span className="text-dim">Not published</span>
            ) : null}
            <span className="flex-1" />
            {busy && (
                <span className="flex min-w-0 items-center gap-2">
                    <span className="h-2 w-2 animate-pulse rounded-full bg-accent" />
                    <span className="truncate">{progress ? `${busy} · ${progress}` : busy}</span>
                </span>
            )}
            <div
                aria-label="Theme"
                className="flex gap-0.5 rounded-md border border-control-line p-0.5"
                role="radiogroup"
            >
                {(['dark', 'light', 'auto'] as Theme[]).map((option) => (
                    <button
                        aria-checked={theme === option}
                        className={`rounded px-1.5 text-[11px] capitalize ${theme === option ? 'bg-tab text-fg' : 'text-muted hover:text-fg'}`}
                        key={option}
                        onClick={() => updateSettings({ theme: option })}
                        role="radio"
                        type="button"
                    >
                        {option}
                    </button>
                ))}
            </div>
            <span className="text-dim">
                {name} {version}
            </span>
        </footer>
    );
}

function ToastView() {
    const { set, toast, undo } = useRepo();

    if (!toast) {
        return null;
    }

    return (
        <div
            className="fixed bottom-12 left-1/2 z-30 flex -translate-x-1/2 items-center gap-3.5 rounded-full border border-control-line bg-panel px-4 py-2 shadow-[0_10px_30px_var(--shadow)]"
            key={toast.id}
        >
            <span>{toast.message}</span>
            {toast.undoable && (
                <button
                    className="font-semibold text-accent hover:text-accent-strong"
                    onClick={() => {
                        set({ toast: null });
                        undo();
                    }}
                    type="button"
                >
                    Undo ⌘Z
                </button>
            )}
        </div>
    );
}

/** A pulsing dot while a recap or a plan is being drawn. */
function VisualBadge() {
    const running = useRepo(
        (state) => state.recap.status === 'running' || state.plan.status === 'running',
    );

    return running ? (
        <span
            aria-label="Visual running"
            className="ml-1.5 inline-block h-1.5 w-1.5 animate-pulse rounded-full bg-accent align-[2px]"
        />
    ) : null;
}

/** A branch another worktree has checked out: opened in that worktree's own window, not switched to. */
function WorktreeItem({
    branch,
    onOpen,
    path,
}: {
    branch: string;
    onOpen(tab?: Tab): void;
    path: string;
}) {
    return (
        <div className="group flex items-center rounded-md hover:bg-hover" title={path}>
            <button
                className="flex min-w-0 flex-1 items-center gap-2 px-2.5 py-1.5 text-left font-mono text-[12px]"
                onClick={() => onOpen()}
                role="menuitem"
                type="button"
            >
                <span className="w-3" />
                <span className="truncate">{branch}</span>
                <span className="shrink-0 rounded border border-control-line px-1 font-sans text-[10.5px] text-muted">
                    worktree
                </span>
            </button>
            <span className="hidden shrink-0 gap-0.5 pr-1.5 group-hover:flex">
                <button
                    className="rounded px-1.5 py-0.5 text-[11.5px] text-muted hover:text-fg"
                    onClick={() => onOpen('review')}
                    role="menuitem"
                    type="button"
                >
                    Review
                </button>
                <button
                    className="rounded px-1.5 py-0.5 text-[11.5px] text-muted hover:text-fg"
                    onClick={() => onOpen('visual')}
                    role="menuitem"
                    type="button"
                >
                    Visual
                </button>
            </span>
        </div>
    );
}
