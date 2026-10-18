import { getCurrentWindow } from '@tauri-apps/api/window';
import { create } from 'zustand';

import type {
    Commit,
    Detail,
    FileChange,
    FileDiff,
    Finding,
    HistoryMeta,
    InProgress,
    LineAction,
    Op,
    Provider,
    RepoStatus,
    ReviewState,
    Stash,
    VisualKind,
    VisualState,
    VisualSummary,
} from './types';

import { api, errorMessage } from './api';
import { getSettings } from './settings';

export type HistoryDetail =
    | { kind: 'combined'; shas: string[] }
    | { kind: 'commit'; sha: string }
    | { kind: 'stash'; stash: Stash };

export type Picker = 'branch' | 'branchDetached' | 'branchFrom' | 'workspace';

/** A finding the diff should scroll to; `nonce` re-triggers the same one. */
export interface ReviewFocus {
    file: string;
    line: null | number;
    nonce: number;
}

export type Sheet =
    | { changes: FileChange[]; kind: 'discard' }
    | { commit: Commit; kind: 'reset' }
    | { commit: Commit; kind: 'reword' }
    | { commit: Commit; kind: 'tag' }
    | { from: null | string; kind: 'newBranch' }
    | { kind: 'deleteBranch'; name: string }
    | { kind: 'drop'; shas: string[] }
    | { kind: 'renameBranch'; name: string; upstream: null | string }
    | { kind: 'squash'; shas: string[] }
    | { kind: 'stash'; paths: string[] };

export type Tab = 'changes' | 'history' | 'review' | 'visual';

export interface Toast {
    id: number;
    message: string;
    undoable: boolean;
}

const emptyStatus: RepoStatus = {
    branch: { ahead: 0, behind: 0, head: null, oid: null, upstream: null },
    conflicted: [],
    staged: [],
    unstaged: [],
    untracked: [],
};

const PAGE = 200;

export type RepoStore = Actions & State;

interface Actions {
    applyLines(action: LineAction, hunks?: number[]): void;
    cancelReview(): void;
    cancelVisual(kind: VisualKind): void;
    commit(andPush?: boolean): void;
    dismissFinding(id: number): void;
    fetch(): void;
    focusFinding(finding: Finding): void;
    init(root: string): Promise<void>;
    loadDiff(): Promise<void>;
    loadMore(): Promise<void>;
    loadReview(): Promise<void>;
    loadVisuals(): Promise<void>;
    /** A toast with no undo. */
    notify(message: string): void;
    /** Shows a saved recap or plan, from any branch, until the branch changes. */
    openVisual(kind: VisualKind, id: string): void;
    /** Opens another worktree's window, closing this one when Settings say so. */
    openWorktree(path: string, tab?: Tab): void;
    perform(op: Op, label: string): void;
    pull(): void;
    push(force?: boolean): void;
    redo(): void;
    refreshHistory(): Promise<void>;
    refreshStatus(): Promise<void>;
    select(change: FileChange | null): void;

    set(patch: Partial<State>): void;
    setAmend(amend: boolean): void;
    /** Ticks commits in History; two or more show their combined diff. */
    setChecked(shas: string[]): void;
    showDetail(detail: HistoryDetail | null): void;
    /** Starts an AI review of the current branch with `provider`, or the one chosen in Settings. */
    startReview(provider?: Provider): void;
    /** Starts a visual recap of the branch, or a plan of the change `prompt` describes. */
    startVisual(kind: VisualKind, prompt?: string, provider?: Provider): void;
    toggleLine(hunk: number, line: number, range: boolean): void;
    undo(): void;
    viewHistory(ref: string): void;
}

interface State {
    amend: boolean;
    baseBranch: null | string;
    baseCommit: null | string;
    body: string;
    busy: null | string;
    checked: string[];
    commits: Commit[];
    detail: HistoryDetail | null;

    detailData: Detail | null;
    diff: FileDiff | null;
    /** Findings hidden in this window, by id, for the review being shown. */
    dismissed: number[];
    error: null | string;
    /** Everything but the diff slides away (⇧⌘E, esc to come back). */
    expanded: boolean;
    hasMore: boolean;
    historyRef: string;
    inProgress: InProgress | null;
    lastCommit: Commit | null;
    lineSelection: Record<number, number[]>;
    loaded: boolean;
    /** The main checkout, when this window shows a linked worktree of it. */
    mainWorktree: null | string;
    meta: HistoryMeta;
    palette: boolean;
    picker: null | Picker;

    plan: VisualState;
    plans: VisualSummary[];
    progress: null | string;
    recap: VisualState;
    recaps: VisualSummary[];
    review: ReviewState;
    reviewFocus: null | ReviewFocus;
    root: string;
    savedMessage: [string, string] | null;
    selected: FileChange | null;
    sheet: null | Sheet;

    status: RepoStatus;
    summary: string;
    tab: Tab;
    toast: null | Toast;
    visualKind: VisualKind;
}

let queue: Promise<void> = Promise.resolve();
let toastTimer: number | undefined;
let anchor: { hunk: number; line: number } | null = null;

export const useRepo = create<RepoStore>()((set, get) => {
    /** Runs `task` after any action already running, then refreshes. */
    function enqueue(
        label: string,
        task: () => Promise<{ message: string; undoable: boolean } | null>,
        refreshHistory = true,
    ) {
        queue = queue.then(async () => {
            set({ busy: label, progress: null });

            try {
                const outcome = await task();

                if (outcome) {
                    showToast(outcome.message, outcome.undoable);
                }
            } catch (e) {
                set({ error: errorMessage(e) });
            } finally {
                set({ busy: null, progress: null });
            }

            await get().refreshStatus();

            if (refreshHistory) {
                await get().refreshHistory();
            }
        });
    }

    function showToast(message: string, undoable: boolean) {
        window.clearTimeout(toastTimer);
        set({ toast: { id: Date.now(), message, undoable } });
        toastTimer = window.setTimeout(() => set({ toast: null }), 5000);
    }

    return {
        amend: false,
        applyLines(action, hunks) {
            const { diff, lineSelection } = get();

            if (!diff) {
                return;
            }

            let selection: Record<number, number[]>;

            if (hunks) {
                selection = Object.fromEntries(
                    diff.hunks
                        .filter((hunk) => hunks.includes(hunk.id))
                        .map((hunk) => [
                            hunk.id,
                            hunk.lines
                                .filter(
                                    (line) => line.kind === 'addition' || line.kind === 'deletion',
                                )
                                .map((line) => line.id),
                        ]),
                );
            } else {
                selection = lineSelection;
            }

            if (!Object.keys(selection).length) {
                return;
            }

            set({ lineSelection: {} });

            const label =
                action === 'stage' ? 'Staging' : action === 'unstage' ? 'Unstaging' : 'Discarding';

            get().perform({ action, file: diff, selection, type: 'applyLines' }, label);
        },
        baseBranch: null,
        baseCommit: null,
        body: '',
        busy: null,
        cancelReview() {
            void api.cancelReview(get().root);
        },
        cancelVisual(kind) {
            void api.cancelVisual(get().root, kind);
        },
        checked: [],
        commit(andPush = false) {
            const { amend, body, busy, status, summary } = get();
            const s = summary.trim();

            if (!s || (!amend && !status.staged.length) || status.conflicted.length || busy) {
                return;
            }

            const b = body.trim();

            get().perform(
                { amend, message: b ? `${s}\n\n${b}` : s, type: 'commit' },
                amend ? 'Amending' : 'Committing',
            );
            set({ amend: false, body: '', savedMessage: null, summary: '' });

            if (andPush) {
                get().push();
            }
        },
        commits: [],
        detail: null,
        detailData: null,
        diff: null,
        dismissed: [],

        dismissFinding(id) {
            set({ dismissed: [...get().dismissed, id] });
        },
        error: null,
        expanded: false,
        fetch() {
            get().perform({ type: 'fetch' }, 'Fetching');
        },

        focusFinding(finding) {
            set({ reviewFocus: { file: finding.file, line: finding.line, nonce: Date.now() } });
        },

        hasMore: false,

        historyRef: 'HEAD',

        async init(root) {
            set({ root });
            await get().refreshStatus();
            set({ loaded: true });
            set({ mainWorktree: await api.mainWorktree(root) });
            await get().refreshHistory();
            await get().loadReview();
            await get().loadVisuals();
        },

        inProgress: null,

        lastCommit: null,

        lineSelection: {},

        async loadDiff() {
            const { root, selected } = get();

            if (!selected || selected.area === 'conflicted') {
                set({ diff: null });

                return;
            }

            const source =
                selected.area === 'staged'
                    ? 'staged'
                    : selected.area === 'untracked'
                      ? 'untracked'
                      : 'unstaged';

            try {
                const diff = await api.fileDiff(
                    root,
                    selected.path,
                    source,
                    getSettings().ignoreWhitespace,
                );

                // Ignore a response for a file that is no longer selected.
                if (get().selected !== selected) {
                    return;
                }

                const keep = Object.fromEntries(
                    Object.entries(get().lineSelection).filter(([h]) =>
                        diff?.hunks.some((candidate) => candidate.id === Number(h)),
                    ),
                );

                set({ diff, lineSelection: keep });
            } catch (e) {
                set({ diff: null, error: errorMessage(e) });
            }
        },
        loaded: false,

        async loadMore() {
            const { commits, hasMore, historyRef, root } = get();

            if (!hasMore) {
                return;
            }

            const more = await api.log(root, historyRef, commits.length, PAGE);

            set({ commits: [...commits, ...more], hasMore: more.length === PAGE });
        },
        async loadReview() {
            try {
                const review = await api.reviewState(get().root);
                const previous = get().review;
                // A new review brings back findings dismissed from the previous one.
                const fresh =
                    review.status === 'done' &&
                    (previous.status !== 'done' ||
                        previous.review.createdAt !== review.review.createdAt);

                set(fresh ? { dismissed: [], review } : { review });
            } catch (error) {
                set({ error: errorMessage(error) });
            }
        },
        async loadVisuals() {
            const root = get().root;

            try {
                const [recap, plan, recaps, plans] = await Promise.all([
                    api.visualState(root, 'recap'),
                    api.visualState(root, 'plan'),
                    api.visualHistory(root, 'recap'),
                    api.visualHistory(root, 'plan'),
                ]);

                set({ plan, plans, recap, recaps });
            } catch (error) {
                set({ error: errorMessage(error) });
            }
        },
        mainWorktree: null,
        meta: { branches: [], remotes: [], stashes: [], tags: [], unpushed: [] },
        notify(message) {
            showToast(message, false);
        },
        openVisual(kind, id) {
            set({ tab: 'visual', visualKind: kind });
            api.openVisual(get().root, kind, id).catch((error) =>
                set({ error: errorMessage(error) }),
            );
        },

        openWorktree(path, tab) {
            api.openRepository(path, tab).then(
                () => {
                    if (getSettings().worktreeReplacesWindow) {
                        void getCurrentWindow().close();
                    }
                },
                (error: unknown) => get().notify(errorMessage(error)),
            );
        },
        palette: false,

        perform(op, label) {
            const local =
                op.type.startsWith('stage') ||
                op.type.startsWith('unstage') ||
                op.type === 'discard' ||
                op.type === 'applyLines';

            enqueue(label, () => api.perform(get().root, op), !local);
        },
        picker: null,
        plan: { status: 'idle' },
        plans: [],
        progress: null,
        pull() {
            get().perform({ mode: getSettings().pullMode, type: 'pull' }, 'Pulling');
        },
        push(force = false) {
            get().perform({ force, type: 'push' }, force ? 'Force pushing' : 'Pushing');
        },
        recap: { status: 'idle' },

        recaps: [],

        redo() {
            enqueue('Redoing', () => api.redo(get().root));
        },
        async refreshHistory() {
            const { root } = get();

            try {
                const meta = await api.historyMeta(root);
                let { historyRef } = get();

                if (
                    historyRef !== 'HEAD' &&
                    !meta.branches.some((branch) => branch.fullName === historyRef)
                ) {
                    historyRef = 'HEAD';
                }

                const commits = await api.log(root, historyRef, 0, PAGE);
                const checked = get().checked.filter((sha) =>
                    commits.some((commit) => commit.sha === sha),
                );

                set({ checked, commits, hasMore: commits.length === PAGE, historyRef, meta });
                await findBase();

                const first = commits[0] ? { kind: 'commit' as const, sha: commits[0].sha } : null;
                let detail = get().detail ?? first;

                // A combined diff follows the ticked commits, which an action may have rewritten or removed.
                if (detail?.kind === 'combined') {
                    detail = checked.length >= 2 ? { kind: 'combined', shas: checked } : first;
                }

                get().showDetail(detail);
            } catch (e) {
                set({ error: errorMessage(e) });
            }
        },
        async refreshStatus() {
            const { root } = get();

            try {
                const snap = await api.snapshot(root);
                const all = [
                    ...snap.status.conflicted,
                    ...snap.status.staged,
                    ...snap.status.unstaged,
                    ...snap.status.untracked,
                ];
                const prev = get().selected;
                // Keep the selection; follow a file that moved between areas (e.g. it was just staged).
                const selected =
                    (prev &&
                        (all.find(
                            (change) => change.path === prev.path && change.area === prev.area,
                        ) ??
                            all.find((change) => change.path === prev.path))) ??
                    all[0] ??
                    null;

                set({
                    inProgress: snap.inProgress,
                    lastCommit: snap.lastCommit,
                    selected,
                    status: snap.status,
                });
                await get().loadDiff();
            } catch (e) {
                set({ error: errorMessage(e) });
            }
        },
        review: { status: 'idle' },

        reviewFocus: null,

        root: '',

        savedMessage: null,

        select(change) {
            anchor = null;
            set({
                diff: change?.path === get().selected?.path ? get().diff : null,
                lineSelection: {},
                selected: change,
            });
            void get().loadDiff();
        },

        selected: null,

        set: (patch) => set(patch),

        setAmend(amend) {
            const { body, lastCommit, savedMessage, summary } = get();

            if (amend && lastCommit) {
                set({
                    amend,
                    body: lastCommit.body,
                    savedMessage: [summary, body],
                    summary: lastCommit.subject,
                });
            } else if (!amend && savedMessage) {
                set({ amend, body: savedMessage[1], savedMessage: null, summary: savedMessage[0] });
            } else {
                set({ amend });
            }
        },

        setChecked(shas) {
            const { commits, detail } = get();

            set({ checked: shas });

            if (shas.length >= 2) {
                get().showDetail({ kind: 'combined', shas });
            } else if (shas.length === 1) {
                get().showDetail({ kind: 'commit', sha: shas[0] });
            } else if (detail?.kind === 'combined') {
                get().showDetail(commits[0] ? { kind: 'commit', sha: commits[0].sha } : null);
            }
        },

        sheet: null,

        showDetail(detail) {
            set({ detail });

            const { root } = get();

            const load = async () => {
                if (!detail) {
                    return null;
                }

                const ws = getSettings().ignoreWhitespace;

                switch (detail.kind) {
                    case 'combined': {
                        const ordered = get().commits.filter((commit) =>
                            detail.shas.includes(commit.sha),
                        );

                        if (!ordered.length) {
                            return null;
                        }

                        return api.rangeDetail(
                            root,
                            ordered[ordered.length - 1].sha,
                            ordered[0].sha,
                        );
                    }

                    case 'commit':
                        return api.commitDetail(root, detail.sha, ws);
                    case 'stash':
                        return api.stashDetail(root, detail.stash);
                }
            };

            load()
                .then((detailData) => {
                    if (get().detail === detail) {
                        set({ detailData });
                    }
                })
                .catch((error) => set({ error: errorMessage(error) }));
        },

        startReview(provider) {
            set({ tab: 'review' });
            // A review that can't start is recorded as failed and shown in the Review tab.
            api.startReview(get().root, provider ?? null).catch(() => get().loadReview());
        },
        startVisual(kind, prompt, provider) {
            set({ tab: 'visual', visualKind: kind });
            // One that can't start is recorded as failed and shown in the Visual tab.
            api.startVisual(get().root, kind, prompt ?? null, provider ?? null).catch(() =>
                get().loadVisuals(),
            );
        },

        status: emptyStatus,

        summary: '',

        tab: 'changes',
        toast: null,

        toggleLine(hunkId, lineId, range) {
            const { diff, lineSelection } = get();
            const hunk = diff?.hunks.find((candidate) => candidate.id === hunkId);

            if (!hunk) {
                return;
            }

            const current = new Set(lineSelection[hunkId] ?? []);

            if (range && anchor && anchor.hunk === hunkId) {
                // Shift-click selects every change line between the anchor and this one.
                const [lo, hi] = [Math.min(anchor.line, lineId), Math.max(anchor.line, lineId)];

                hunk.lines.forEach((line) => {
                    if (
                        (line.kind === 'addition' || line.kind === 'deletion') &&
                        line.id >= lo &&
                        line.id <= hi
                    ) {
                        current.add(line.id);
                    }
                });
            } else if (current.has(lineId)) {
                current.delete(lineId);
            } else {
                current.add(lineId);
            }

            anchor = { hunk: hunkId, line: lineId };

            const next = { ...lineSelection };

            if (current.size) {
                next[hunkId] = [...current];
            } else {
                delete next[hunkId];
            }

            set({ lineSelection: next });
        },

        undo() {
            enqueue('Undoing', () => api.undo(get().root));
        },

        viewHistory(ref) {
            set({ checked: [], detail: null, historyRef: ref, tab: 'history' });
            void get().refreshHistory();
        },

        visualKind: 'recap',
    };

    async function findBase() {
        const { historyRef, meta, root, status } = get();
        const head =
            historyRef === 'HEAD' ? status.branch.head : historyRef.replace('refs/heads/', '');
        const local = new Set(
            meta.branches.filter((branch) => !branch.isRemote).map((branch) => branch.name),
        );
        const remote = new Set(
            meta.branches.filter((branch) => branch.isRemote).map((branch) => branch.name),
        );
        const candidates = ['main', 'master', 'develop', 'trunk'];
        const base =
            candidates.find((candidate) => local.has(candidate) && candidate !== head) ??
            candidates
                .map((candidate) => `origin/${candidate}`)
                .find((name) => remote.has(name) && name !== `origin/${head}`);

        if (!base) {
            set({ baseBranch: null, baseCommit: null });

            return;
        }

        set({ baseBranch: base, baseCommit: await api.mergeBase(root, historyRef, base) });
    }
});

export const isChange = (kind: string) => kind === 'addition' || kind === 'deletion';

export function branchLabel(status: RepoStatus) {
    return (
        status.branch.head ??
        (status.branch.oid ? `detached at ${status.branch.oid.slice(0, 7)}` : 'no branch')
    );
}

export function changeCount(status: RepoStatus) {
    return new Set(
        [...status.staged, ...status.unstaged, ...status.untracked, ...status.conflicted].map(
            (change) => change.path,
        ),
    ).size;
}

export function isPushed(meta: HistoryMeta, sha: string) {
    return !meta.unpushed.includes(sha);
}

export function repoName(root: string) {
    return root.split('/').filter(Boolean).pop() ?? root;
}
