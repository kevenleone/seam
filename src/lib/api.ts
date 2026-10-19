import { invoke } from '@tauri-apps/api/core';

import type { Tab } from './store';
import type {
    AiConfig,
    CliStatus,
    Commit,
    Detail,
    DiffSource,
    Drift,
    Facts,
    FileDiff,
    HistoryMeta,
    NotifyPrefs,
    Op,
    Outcome,
    OverviewState,
    OverviewSummary,
    Provider,
    ProviderStatus,
    ReviewState,
    ScopeSummary,
    Snapshot,
    Stash,
    Target,
    TestResult,
    VisualKind,
    VisualState,
    VisualSummary,
    Worktree,
} from './types';

/** Typed wrappers around the Rust commands in src-tauri/src/lib.rs. */
export const api = {
    aiConfig: () => invoke<AiConfig>('ai_config'),
    aiProviders: () => invoke<ProviderStatus[]>('ai_providers'),
    /** A blob's bytes; `path` lets an unwritten working-tree side be read from disk. */
    blob: (root: string, object: string, path: null | string) =>
        invoke<ArrayBuffer>('blob', { object, path, root }),
    cancelOverview: (root: string) => invoke<void>('cancel_overview', { root }),
    cancelReview: (root: string) => invoke<void>('cancel_review', { root }),
    cancelVisual: (root: string, kind: VisualKind) => invoke<void>('cancel_visual', { kind, root }),
    clearAiKey: (provider: Provider) => invoke<void>('clear_ai_key', { provider }),
    cliStatus: () => invoke<CliStatus>('cli_status'),
    cloneRepository: (url: string, destination: string) =>
        invoke<void>('clone_repository', { destination, url }),
    /** Back to the review shown before a change was opened from the terminal. */
    closeOpenedReview: (root: string) => invoke<void>('close_opened_review', { root }),
    commitDetail: (root: string, sha: string, ignoreWhitespace: boolean) =>
        invoke<Detail>('commit_detail', { ignoreWhitespace, root, sha }),
    /** Resolves `false` when the save panel was dismissed. */
    exportVisual: (name: string, extension: string, contents: string) =>
        invoke<boolean>('export_visual', { contents, extension, name }),
    fileDiff: (root: string, path: string, source: DiffSource, ignoreWhitespace: boolean) =>
        invoke<FileDiff | null>('file_diff', { ignoreWhitespace, path, root, source }),
    gitVersion: () => invoke<string>('git_version'),
    historyMeta: (root: string) => invoke<HistoryMeta>('history_meta', { root }),
    importRecentRepositories: (paths: string[]) =>
        invoke<void>('import_recent_repositories', { paths }),
    initRepository: (path: string) => invoke<void>('init_repository', { path }),
    log: (root: string, reference: string, skip: number, limit: number) =>
        invoke<Commit[]>('log', { limit, reference, root, skip }),
    mainWorktree: (root: string) => invoke<null | string>('main_worktree', { root }),
    mergeBase: (root: string, first: string, second: string) =>
        invoke<null | string>('merge_base', { a: first, b: second, root }),
    notifyPrefs: () => invoke<NotifyPrefs>('notify_prefs'),
    openFolderDialog: () => invoke<void>('open_folder_dialog'),
    openOverview: (root: string, id: string) => invoke<void>('open_overview', { id, root }),
    openRepository: (path: string, tab?: Tab) =>
        invoke<void>('open_repository', { path, tab: tab ?? null }),
    openSettingsWindow: (pane: null | string) => invoke<void>('open_settings_window', { pane }),
    openVisual: (root: string, kind: VisualKind, id: string) =>
        invoke<void>('open_visual', { id, kind, root }),
    openWelcome: () => invoke<void>('open_welcome'),
    /** `null` before the first overview is written. */
    overviewDrift: (root: string) => invoke<Drift | null>('overview_drift', { root }),
    overviewFacts: (root: string) => invoke<Facts>('overview_facts', { root }),
    overviewHistory: (root: string) => invoke<OverviewSummary[]>('overview_history', { root }),
    overviewState: (root: string) => invoke<OverviewState>('overview_state', { root }),
    perform: (root: string, op: Op) => invoke<Outcome>('perform', { op, root }),
    playSound: (sound: string) => invoke<void>('play_sound', { sound }),
    printWindow: () => invoke<void>('print_window'),
    rangeDetail: (root: string, oldest: string, newest: string) =>
        invoke<Detail>('range_detail', { newest, oldest, root }),
    recentRepositories: () => invoke<string[]>('recent_repositories'),
    redo: (root: string) => invoke<null | Outcome>('redo', { root }),
    removeRecentRepository: (path: string) => invoke<void>('remove_recent_repository', { path }),
    reviewState: (root: string) => invoke<ReviewState>('review_state', { root }),
    setAiConfig: (config: AiConfig) => invoke<void>('set_ai_config', { config }),

    setAiKey: (provider: Provider, key: string) => invoke<void>('set_ai_key', { key, provider }),
    setCliInstalled: (installed: boolean) => invoke<CliStatus>('set_cli_installed', { installed }),
    setGitPath: (path: null | string) => invoke<void>('set_git_path', { path }),
    setNotifyPrefs: (notify: NotifyPrefs) => invoke<void>('set_notify_prefs', { notify }),
    snapshot: (root: string) => invoke<Snapshot>('snapshot', { root }),
    sounds: () => invoke<string[]>('sounds'),
    /** With `sections`, rewrites only those of the overview shown. */
    startOverview: (root: string, sections: null | string[], provider: null | Provider) =>
        invoke<void>('start_overview', { provider, root, sections }),
    startReview: (root: string, provider: null | Provider, target: null | Target) =>
        invoke<void>('start_review', { provider, root, target }),
    startVisual: (
        root: string,
        kind: VisualKind,
        prompt: null | string,
        provider: null | Provider,
    ) => invoke<void>('start_visual', { kind, prompt, provider, root }),
    stashDetail: (root: string, stash: Stash) => invoke<Detail>('stash_detail', { root, stash }),
    testAi: () => invoke<TestResult>('test_ai'),

    topLevel: (path: string) => invoke<null | string>('top_level', { path }),
    undo: (root: string) => invoke<null | Outcome>('undo', { root }),
    visualHistory: (root: string, kind: VisualKind) =>
        invoke<VisualSummary[]>('visual_history', { kind, root }),
    visualScope: (root: string) => invoke<ScopeSummary>('visual_scope', { root }),
    visualState: (root: string, kind: VisualKind) =>
        invoke<VisualState>('visual_state', { kind, root }),
    watch: (root: string) => invoke<void>('watch', { root }),
    worktrees: (root: string) => invoke<Worktree[]>('worktrees', { root }),
};

export function errorMessage(error: unknown): string {
    if (typeof error === 'string') {
        return error;
    }

    if (error instanceof Error) {
        return error.message;
    }

    return String(error);
}
