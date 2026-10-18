import { listen } from '@tauri-apps/api/event';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { openUrl, revealItemInDir } from '@tauri-apps/plugin-opener';
import { lazy, Suspense, useEffect } from 'react';

import { Icon } from '../components/Icon';
import { Palette } from '../components/Palette';
import { BranchPicker, WorkspacePicker } from '../components/Pickers';
import { SettingsModal } from '../components/settings/SettingsModal';
import { Sheets } from '../components/Sheets';
import { Button } from '../components/ui';
import { api } from '../lib/api';
import { DOCUMENTATION_URL, ISSUES_URL } from '../lib/links';
import {
    effectiveMode,
    getSettings,
    syncRecents,
    updateSettings,
    useSettings,
} from '../lib/settings';
import { useSettingsModal } from '../lib/settingsModal';
import { branchLabel, changeCount, repoName, type Tab, useRepo } from '../lib/store';
import { useAppName, useAppVersion } from '../lib/version';
import { ChangesView } from './ChangesView';
import { HistoryView } from './HistoryView';
import { ReviewView } from './ReviewView';

// Their diagrams, markdown and sanitiser load the first time either tab opens.
const OverviewView = lazy(() =>
    import('./OverviewView').then((module) => ({ default: module.OverviewView })),
);
const VisualView = lazy(() =>
    import('./VisualView').then((module) => ({ default: module.VisualView })),
);

export function RepoWindow({ root }: { root: string }) {
    const tab = useRepo((state) => state.tab);
    const loaded = useRepo((state) => state.loaded);
    const palette = useRepo((state) => state.palette);
    const picker = useRepo((state) => state.picker);
    const expanded = useRepo((state) => state.expanded);
    const { statusBar } = useSettings();

    useEffect(() => {
        void syncRecents();
        void getCurrentWebviewWindow().setTitle(repoName(root));

        // The tray opens a window straight onto its review with ?tab=review.
        const initialTab = new URLSearchParams(location.search).get('tab');
        const tabs: Tab[] = ['changes', 'history', 'overview', 'review', 'visual'];

        void useRepo.getState().init(
            root,
            tabs.find((candidate) => candidate === initialTab),
        );
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
                    void s.loadOverview();
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
            listen<string>('overview-updated', (event) => {
                if (event.payload === root) {
                    void useRepo.getState().loadOverview();
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
                {tab === 'overview' ? (
                    <Suspense fallback={null}>
                        <OverviewView />
                    </Suspense>
                ) : tab === 'changes' ? (
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
            {picker === 'workspace' && <WorkspacePicker />}
            {picker && picker !== 'workspace' && <BranchPicker key={picker} mode={picker} />}
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

    return (
        <header
            className="flex min-h-[46px] shrink-0 items-center gap-3 overflow-hidden border-b border-line bg-panel py-1.5 pr-3.5 pl-[84px] whitespace-nowrap"
            data-tauri-drag-region
        >
            <PanelToggle side="left" />
            <span
                className="max-w-[220px] min-w-0 shrink truncate font-semibold"
                data-tauri-drag-region
                title={s.root}
            >
                {repoName(s.root)}
            </span>
            {s.mainWorktree && <WorktreeChip main={s.mainWorktree} root={s.root} />}
            <button
                aria-haspopup="listbox"
                className="flex h-7 max-w-[160px] min-w-0 shrink items-center gap-1.5 rounded-md border border-control-line bg-control px-2.5 font-mono text-[12px] xl:max-w-[300px]"
                onClick={() => s.set({ picker: 'branch' })}
                title={`${branchLabel(s.status)}: switch branch`}
                type="button"
            >
                <Icon className="shrink-0 text-muted" name="branch" size={12} />
                <span className="truncate">{branchLabel(s.status)}</span>
                <Icon className="shrink-0 text-muted" name="chevronDown" size={11} />
            </button>
            <nav
                aria-label="Views"
                className="flex shrink-0 gap-0.5 rounded-[7px] border border-control-line bg-control p-0.5"
            >
                {(['overview', 'changes', 'history', 'review', 'visual'] as const).map((option) => (
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
                                overview: 'Overview',
                                review: 'Review',
                                visual: 'Visual',
                            }[option]
                        }
                        {option === 'changes' && count > 0 && (
                            <span className="ml-1.5 text-accent">{count}</span>
                        )}
                        {option === 'review' && <ReviewBadge />}
                        {option === 'overview' && <OverviewBadge />}
                        {option === 'visual' && <VisualBadge />}
                    </button>
                ))}
            </nav>
            <div className="min-w-2 flex-1" data-tauri-drag-region />
            <Button
                aria-label="Fetch"
                className="shrink-0"
                onClick={s.fetch}
                size="sm"
                title="Fetch from all remotes (⇧⌘F)"
                variant="ghost"
            >
                <Icon name="refresh" size={13} />
            </Button>
            <Button className="shrink-0" onClick={s.pull} size="sm" title="Pull (⇧⌘P)">
                <Icon name="down" size={12} /> <span className="hidden lg:inline">Pull</span>
                {behind > 0 && <span className="text-accent">↓{behind}</span>}
            </Button>
            <Button
                className="shrink-0"
                onClick={() => s.push()}
                size="sm"
                title={upstream ? `Push to ${upstream} (⌘P)` : 'Publish this branch (⌘P)'}
            >
                <Icon name="up" size={12} />{' '}
                <span className="hidden lg:inline">{upstream ? 'Push' : 'Publish'}</span>
                {ahead > 0 && <span className="text-accent">↑{ahead}</span>}
            </Button>
            <Button
                aria-label="Command palette"
                className="shrink-0"
                onClick={() => s.set({ palette: true })}
                size="sm"
                title="Search and run any command (⌘K)"
                variant="ghost"
            >
                <Icon name="search" size={13} />{' '}
                <span className="hidden font-mono text-[11px] lg:inline">⌘K</span>
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
        case 'switch-branch':
            return s.set({ picker: 'branch' });
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
        case 'view-overview':
            return s.set({ tab: 'overview' });
        case 'view-review':
            return s.set({ tab: 'review' });
        case 'view-visual':
            return s.set({ tab: 'visual' });
    }
}

/** A pulsing dot while the overview is written, then a dot while it is out of date. */
function OverviewBadge() {
    const running = useRepo((state) => state.overview.status === 'running');
    const drifted = useRepo((state) => !!state.drift?.sections.length);

    if (running) {
        return (
            <span
                aria-label="Overview being written"
                className="ml-1.5 inline-block h-1.5 w-1.5 animate-pulse rounded-full bg-accent align-[2px]"
            />
        );
    }

    return drifted ? (
        <span
            aria-label="Overview out of date"
            className="ml-1.5 inline-block h-1.5 w-1.5 rounded-full bg-del align-[2px]"
        />
    ) : null;
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

const SEGMENT =
    'flex min-w-0 items-center gap-[5px] rounded-md px-1.5 py-[3px] text-muted hover:bg-hover hover:text-fg';

const ICON_SEGMENT = `${SEGMENT} h-[22px] w-[22px] shrink-0 justify-center px-0`;

function StatusBar() {
    const name = useAppName();
    const version = useAppVersion();
    const { busy, fetch, progress, push, root, set, status } = useRepo();
    const { theme } = useSettings();
    const showSettings = useSettingsModal((state) => state.show);
    const { ahead, behind, head, upstream } = status.branch;
    const dark = effectiveMode(theme) === 'dark';

    return (
        <footer className="flex h-8 shrink-0 items-center gap-0.5 overflow-hidden border-t border-line bg-panel px-2 text-[12px] whitespace-nowrap text-muted">
            <button
                className={SEGMENT}
                onClick={() => set({ picker: 'workspace' })}
                title={`${root}: open another workspace`}
                type="button"
            >
                <Icon className="shrink-0" name="folder" size={12} />
                <span className="max-w-[200px] truncate">{repoName(root)}</span>
            </button>
            <StatusDivider />
            <button
                className={SEGMENT}
                onClick={() => set({ picker: 'branch' })}
                title="Switch branch"
                type="button"
            >
                <Icon className="shrink-0" name="branch" size={12} />
                <span className="max-w-[240px] truncate font-mono text-[11.5px]">
                    {branchLabel(status)}
                </span>
            </button>
            {upstream ? (
                <button
                    className={`${SEGMENT} shrink-0 font-mono text-[11.5px]`}
                    onClick={fetch}
                    title={`Behind and ahead of ${upstream}. Click to fetch.`}
                    type="button"
                >
                    ↓{behind} ↑{ahead}
                </button>
            ) : head ? (
                <button className={`${SEGMENT} shrink-0`} onClick={() => push()} type="button">
                    <Icon name="up" size={11} /> Publish branch
                </button>
            ) : null}
            <span className="min-w-2 flex-1" />
            {busy && (
                <span className="mr-2 flex min-w-0 items-center gap-2">
                    <span className="h-2 w-2 shrink-0 animate-pulse rounded-full bg-accent" />
                    <span className="truncate">{progress ? `${busy} · ${progress}` : busy}</span>
                </span>
            )}
            <button
                className={`${SEGMENT} hidden shrink-0 md:flex`}
                onClick={() => void openUrl(ISSUES_URL)}
                type="button"
            >
                <Icon name="external" size={11} /> Contribute
            </button>
            <button
                className={`${SEGMENT} hidden shrink-0 md:flex`}
                onClick={() => void openUrl(DOCUMENTATION_URL)}
                type="button"
            >
                <Icon name="external" size={11} /> Documentation
            </button>
            <StatusDivider />
            <button
                aria-label={dark ? 'Switch to light' : 'Switch to dark'}
                className={ICON_SEGMENT}
                onClick={() => updateSettings({ theme: dark ? 'light' : 'dark' })}
                title={dark ? 'Switch to light' : 'Switch to dark'}
                type="button"
            >
                <Icon name={dark ? 'sun' : 'moon'} size={13} />
            </button>
            <button
                aria-label="Settings"
                className={ICON_SEGMENT}
                onClick={() => showSettings()}
                title="Settings (⌘,)"
                type="button"
            >
                <Icon name="gear" size={13} />
            </button>
            <StatusDivider />
            <button
                className={`${SEGMENT} shrink-0`}
                onClick={() => showSettings('about')}
                title="About"
                type="button"
            >
                {name} {version}
            </button>
        </footer>
    );
}

function StatusDivider() {
    return <span className="mx-0.5 h-[13px] w-px shrink-0 bg-line" />;
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

/** Marks a window showing a linked worktree; opens the checkout it belongs to. */
function WorktreeChip({ main, root }: { main: string; root: string }) {
    const openWorktree = useRepo((state) => state.openWorktree);

    return (
        <button
            className="-ml-1 flex shrink-0 items-center gap-1.5 rounded-md border border-control-line px-1.5 py-0.5 text-[11.5px] text-muted hover:bg-hover hover:text-fg xl:px-2"
            onClick={() => openWorktree(main)}
            title={`${root} is a worktree of ${main}. Click to open ${repoName(main)}.`}
            type="button"
        >
            <Icon name="folder" size={11} />
            <span className="hidden xl:inline">worktree of {repoName(main)}</span>
        </button>
    );
}
