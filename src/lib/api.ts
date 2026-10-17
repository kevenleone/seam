import { invoke } from '@tauri-apps/api/core';

import type {
    AiConfig,
    Commit,
    Detail,
    DiffSource,
    FileDiff,
    HistoryMeta,
    Op,
    Outcome,
    Provider,
    ProviderStatus,
    ReviewState,
    Snapshot,
    Stash,
    TestResult,
} from './types';

/** Typed wrappers around the Rust commands in src-tauri/src/lib.rs. */
export const api = {
    aiConfig: () => invoke<AiConfig>('ai_config'),
    aiProviders: () => invoke<ProviderStatus[]>('ai_providers'),
    cancelReview: (root: string) => invoke<void>('cancel_review', { root }),
    clearAiKey: (provider: Provider) => invoke<void>('clear_ai_key', { provider }),
    cloneRepository: (url: string, destination: string) =>
        invoke<void>('clone_repository', { destination, url }),
    commitDetail: (root: string, sha: string, ignoreWhitespace: boolean) =>
        invoke<Detail>('commit_detail', { ignoreWhitespace, root, sha }),
    fileDiff: (root: string, path: string, source: DiffSource, ignoreWhitespace: boolean) =>
        invoke<FileDiff | null>('file_diff', { ignoreWhitespace, path, root, source }),
    gitVersion: () => invoke<string>('git_version'),
    historyMeta: (root: string) => invoke<HistoryMeta>('history_meta', { root }),
    importRecentRepositories: (paths: string[]) =>
        invoke<void>('import_recent_repositories', { paths }),
    initRepository: (path: string) => invoke<void>('init_repository', { path }),
    log: (root: string, reference: string, skip: number, limit: number) =>
        invoke<Commit[]>('log', { limit, reference, root, skip }),
    mergeBase: (root: string, first: string, second: string) =>
        invoke<null | string>('merge_base', { a: first, b: second, root }),
    openFolderDialog: () => invoke<void>('open_folder_dialog'),
    openRepository: (path: string) => invoke<void>('open_repository', { path }),
    openSettingsWindow: (pane: null | string) => invoke<void>('open_settings_window', { pane }),
    openWelcome: () => invoke<void>('open_welcome'),
    perform: (root: string, op: Op) => invoke<Outcome>('perform', { op, root }),
    rangeDetail: (root: string, oldest: string, newest: string) =>
        invoke<Detail>('range_detail', { newest, oldest, root }),
    recentRepositories: () => invoke<string[]>('recent_repositories'),
    redo: (root: string) => invoke<null | Outcome>('redo', { root }),
    removeRecentRepository: (path: string) => invoke<void>('remove_recent_repository', { path }),
    reviewState: (root: string) => invoke<ReviewState>('review_state', { root }),

    setAiConfig: (config: AiConfig) => invoke<void>('set_ai_config', { config }),
    setAiKey: (provider: Provider, key: string) => invoke<void>('set_ai_key', { key, provider }),
    setGitPath: (path: null | string) => invoke<void>('set_git_path', { path }),
    snapshot: (root: string) => invoke<Snapshot>('snapshot', { root }),
    startReview: (root: string, provider: null | Provider) =>
        invoke<void>('start_review', { provider, root }),
    stashDetail: (root: string, stash: Stash) => invoke<Detail>('stash_detail', { root, stash }),

    testAi: () => invoke<TestResult>('test_ai'),
    topLevel: (path: string) => invoke<null | string>('top_level', { path }),
    undo: (root: string) => invoke<null | Outcome>('undo', { root }),
    watch: (root: string) => invoke<void>('watch', { root }),
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
