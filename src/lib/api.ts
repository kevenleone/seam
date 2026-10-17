import { invoke } from '@tauri-apps/api/core';

import type {
    Commit,
    Detail,
    DiffSource,
    FileDiff,
    HistoryMeta,
    Op,
    Outcome,
    Snapshot,
    Stash,
} from './types';

/** Typed wrappers around the Rust commands in src-tauri/src/lib.rs. */
export const api = {
    cloneRepository: (url: string, destination: string) =>
        invoke<void>('clone_repository', { destination, url }),
    commitDetail: (root: string, sha: string, ignoreWhitespace: boolean) =>
        invoke<Detail>('commit_detail', { ignoreWhitespace, root, sha }),
    fileDiff: (root: string, path: string, source: DiffSource, ignoreWhitespace: boolean) =>
        invoke<FileDiff | null>('file_diff', { ignoreWhitespace, path, root, source }),
    gitVersion: () => invoke<string>('git_version'),
    historyMeta: (root: string) => invoke<HistoryMeta>('history_meta', { root }),
    initRepository: (path: string) => invoke<void>('init_repository', { path }),
    log: (root: string, reference: string, skip: number, limit: number) =>
        invoke<Commit[]>('log', { limit, reference, root, skip }),
    mergeBase: (root: string, first: string, second: string) =>
        invoke<null | string>('merge_base', { a: first, b: second, root }),

    openFolderDialog: () => invoke<void>('open_folder_dialog'),
    openRepository: (path: string) => invoke<void>('open_repository', { path }),
    openWelcome: () => invoke<void>('open_welcome'),
    perform: (root: string, op: Op) => invoke<Outcome>('perform', { op, root }),
    rangeDetail: (root: string, oldest: string, newest: string) =>
        invoke<Detail>('range_detail', { newest, oldest, root }),
    redo: (root: string) => invoke<null | Outcome>('redo', { root }),
    setGitPath: (path: null | string) => invoke<void>('set_git_path', { path }),
    snapshot: (root: string) => invoke<Snapshot>('snapshot', { root }),

    stashDetail: (root: string, stash: Stash) => invoke<Detail>('stash_detail', { root, stash }),
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
