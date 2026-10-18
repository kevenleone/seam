// Mirrors src-tauri/src/git/models.rs (serde camelCase).

export interface AiConfig {
    baseUrl: string;
    model: string;
    provider: Provider;
    review: ReviewStyle;
}
export interface Branch {
    ahead: number;
    author: string;
    behind: number;
    /** The tip's committer date, in seconds. */
    date: number;
    fullName: string;
    isCurrent: boolean;
    isRemote: boolean;
    name: string;
    sha: string;
    subject: string;
    upstream: null | string;
    upstreamGone: boolean;
    worktree: null | string;
}

export interface BranchStatus {
    ahead: number;
    behind: number;
    head: null | string;
    oid: null | string;
    upstream: null | string;
}

export type ChangeArea = 'conflicted' | 'staged' | 'unstaged' | 'untracked';

export type ChangeKind =
    | 'added'
    | 'conflicted'
    | 'copied'
    | 'deleted'
    | 'modified'
    | 'renamed'
    | 'typeChanged'
    | 'untracked';

export interface CliStatus {
    installed: boolean;
    name: string;
}

export interface Commit {
    authorEmail: string;
    authorName: string;
    body: string;
    date: number;
    parents: string[];
    refs: string[];
    sha: string;
    subject: string;
}

export interface CommitFileStat {
    additions: null | number;
    deletions: null | number;
    path: string;
}

export type ConflictSide = 'ours' | 'theirs';

export interface Detail {
    commit: Commit | null;
    diffs: FileDiff[];
    files: CommitFileStat[];
}

export interface DiffLine {
    id: number;
    kind: LineKind;
    newNumber: null | number;
    oldNumber: null | number;
    text: string;
}

export type DiffSource = 'staged' | 'unstaged' | 'untracked';

export interface FileChange {
    area: ChangeArea;
    kind: ChangeKind;
    originalPath: null | string;
    path: string;
}

export interface FileDiff {
    additions: number;
    deletions: number;
    headerLines: string[];
    hunks: Hunk[];
    isBinary: boolean;
    newPath: null | string;
    oldPath: null | string;
}

export interface Finding {
    body: string;
    file: string;
    id: number;
    line: null | number;
    severity: Severity;
    suggestion: null | string;
    title: string;
}

export interface HistoryMeta {
    branches: Branch[];
    remotes: Remote[];
    stashes: Stash[];
    tags: Tag[];
    unpushed: string[];
}

export interface Hunk {
    id: number;
    lines: DiffLine[];
    newCount: number;
    newStart: number;
    oldCount: number;
    oldStart: number;
    section: string;
}

export type InProgress =
    | { headName: null | string; kind: 'rebase'; step: null | number; total: null | number }
    | { kind: 'cherryPick' }
    | { kind: 'merge' }
    | { kind: 'revert' };

export type LineAction = 'discard' | 'stage' | 'unstage';
export type LineKind = 'addition' | 'context' | 'deletion' | 'noNewline';
export interface NotifyPrefs {
    banner: boolean;
    /** A macOS system sound by name, or empty for none. */
    sound: string;
}
/** Every action the backend can perform and undo; mirrors `Op` in ops.rs. */
export type Op =
    | {
          action: LineAction;
          file: FileDiff;
          selection: null | Record<number, number[]>;
          type: 'applyLines';
      }
    | { amend: boolean; message: string; type: 'commit' }
    | { branch: string; type: 'checkout' }
    | { changes: FileChange[]; type: 'discard' }
    | { checkout: boolean; name: string; start: null | string; type: 'createBranch' }
    | { force: boolean; name: string; type: 'deleteBranch' }
    | { force: boolean; type: 'push' }
    | { includeUntracked: boolean; message: null | string; paths: string[]; type: 'stashPush' }
    | { message: null | string; name: string; sha: string; type: 'createTag' }
    | { message: string; sha: string; type: 'reword' }
    | { message: string; shas: string[]; type: 'squash' }
    | { mode: PullMode; type: 'pull' }
    | { mode: ResetMode; sha: string; type: 'reset' }
    | { name: string; type: 'deleteTag' }
    | { new: string; old: string; type: 'renameBranch' }
    | { path: string; side: ConflictSide | null; type: 'resolve' }
    | { paths: string[]; type: 'stage' }
    | { paths: string[]; type: 'unstage' }
    | { pop: boolean; stash: Stash; type: 'stashApply' }
    | { rev: string; type: 'checkoutDetached' }
    | { sha: string; type: 'revert' }
    | { shas: string[]; type: 'cherryPick' }
    | { shas: string[]; type: 'drop' }
    | { stash: Stash; type: 'stashDrop' }
    | { type: 'abort' }
    | { type: 'continue' }
    | { type: 'fetch' }
    | { type: 'skip' }
    | { type: 'stageAll' }
    | { type: 'uncommit' }
    | { type: 'unstageAll' };
export interface Outcome {
    message: string;
    undoable: boolean;
}

export type Provider =
    'anthropic' | 'claude' | 'codex' | 'google' | 'openai-compatible' | 'openai' | 'opencode';

export interface ProviderStatus {
    available: boolean;
    defaultModel: null | string;
    hasKey: boolean;
    id: Provider;
    kind: 'api' | 'harness';
    label: string;
    path: null | string;
}

export type PullMode = 'fastForwardOnly' | 'merge' | 'rebase';

export interface Remote {
    name: string;
    url: string;
}

export interface RepoStatus {
    branch: BranchStatus;
    conflicted: FileChange[];
    staged: FileChange[];
    unstaged: FileChange[];
    untracked: FileChange[];
}

export type ResetMode = 'hard' | 'mixed' | 'soft';

export interface Review {
    commits: number;
    createdAt: number;
    diffs: FileDiff[];
    durationMs: number;
    findings: Finding[];
    headSha: null | string;
    provider: Provider;
    scope: string;
    summary: string;
    truncated: boolean;
}

export type ReviewFocusArea = 'breaking' | 'bugs' | 'performance' | 'security' | 'style' | 'tests';

export type ReviewState =
    | {
          /** `%h %s` lines, newest first. */
          commits: string[];
          diffs: FileDiff[];
          files: number;
          provider: Provider;
          scope: string;
          startedAt: number;
          status: 'running';
      }
    | { message: string; previous: null | Review; provider: Provider; status: 'failed' }
    | { review: Review; status: 'done' }
    | { status: 'idle' };

export interface ReviewStyle {
    focus: ReviewFocusArea[];
    language: string;
    level: 'balanced' | 'blockers' | 'thorough';
}

export interface ScopeSummary {
    additions: number;
    commits: number;
    deletions: number;
    files: number;
    label: string;
}

export type Severity = 'check' | 'note' | 'risk';

export interface Snapshot {
    inProgress: InProgress | null;
    lastCommit: Commit | null;
    status: RepoStatus;
}

export interface Stash {
    date: number;
    index: number;
    message: string;
    sha: string;
}

export interface Tag {
    name: string;
    sha: string;
}

export interface TestResult {
    durationMs: number;
    reply: string;
}

export interface Visual {
    blocks: VisualBlock[];
    /** The branch it was drawn on. */
    branch: null | string;
    brief: string;
    canvas: VisualCanvas;
    commits: number;
    createdAt: number;
    diffs: FileDiff[];
    durationMs: number;
    headSha: null | string;
    id: string;
    kind: VisualKind;
    prompt: null | string;
    provider: Provider;
    scope: string;
    title: string;
    truncated: boolean;
}

export interface VisualAnnotation {
    line: null | number;
    note: string;
    side: string;
}

export type VisualBlock =
    | ({ type: 'code' } & VisualCode)
    | ({ type: 'compare' } & VisualCompare)
    | ({ type: 'contract' } & VisualContract)
    | ({ type: 'decision' } & VisualDecision)
    | ({ type: 'diagram' } & VisualDiagram)
    | ({ type: 'diff' } & VisualDiff)
    | ({ type: 'fileTree' } & VisualFileTree)
    | ({ type: 'questions' } & VisualQuestions)
    | ({ type: 'text' } & VisualText)
    | ({ type: 'verify' } & VisualVerify);

export interface VisualCanvas {
    connectors: { from: string; label: null | string; to: string }[];
    pins: VisualPin[];
    screens: VisualScreen[];
}

/** `added`, `modified`, `removed`, `renamed`, or empty for unchanged. */
export type VisualChange = '' | 'added' | 'modified' | 'removed' | 'renamed';

export interface VisualCode {
    annotations: VisualAnnotation[];
    code: string;
    file: string;
    language: string;
    summary: string;
}

export interface VisualCompare {
    after: VisualSide;
    before: VisualSide;
    title: string;
}

export interface VisualContract {
    endpoints: {
        change: VisualChange;
        method: string;
        path: string;
        request: null | string;
        response: null | string;
        summary: string;
    }[];
    entities: {
        change: VisualChange;
        fields: { change: VisualChange; name: string; type: string; was: null | string }[];
        name: string;
    }[];
    title: string;
}

export interface VisualDecision {
    alternatives: { option: string; whyNot: string }[];
    consequences: string;
    context: string;
    decision: string;
    title: string;
}

export interface VisualDiagram {
    direction: string;
    edges: { change: VisualChange; from: string; label: null | string; to: string }[];
    nodes: VisualNode[];
    title: string;
}

export interface VisualDiff {
    annotations: VisualAnnotation[];
    file: string;
    summary: string;
}

export interface VisualFileTree {
    files: {
        additions: number;
        change: VisualChange;
        deletions: number;
        note: null | string;
        path: string;
    }[];
}

export type VisualKind = 'plan' | 'recap';

export interface VisualNode {
    change: VisualChange;
    detail: null | string;
    group: null | string;
    id: string;
    kind: string;
    label: string;
}

export interface VisualPin {
    body: string;
    number: number;
    screen: string;
    target: string;
    title: string;
}

export interface VisualQuestions {
    items: { question: string; recommendation: string }[];
}

export interface VisualScreen {
    html: string;
    id: string;
    note: null | string;
    state: 'after' | 'before' | 'proposed' | null;
    surface: string;
    title: string;
}

export interface VisualSide {
    code: null | string;
    language: null | string;
    screen: null | string;
    text: null | string;
}

export type VisualState =
    | {
          /** `%h %s` lines, newest first. */
          commits: string[];
          diffs: FileDiff[];
          kind: VisualKind;
          prompt: null | string;
          provider: Provider;
          scope: string;
          startedAt: number;
          status: 'running';
      }
    | {
          kind: VisualKind;
          message: string;
          previous: null | Visual;
          provider: Provider;
          status: 'failed';
      }
    | { status: 'done'; visual: Visual }
    | { status: 'idle' };

/** One saved recap or plan, as its history lists it. */
export interface VisualSummary {
    branch: null | string;
    createdAt: number;
    id: string;
    prompt: string;
    scope: string;
    title: string;
}

export interface VisualText {
    markdown: string;
}

export interface VisualVerify {
    steps: { expect: string; how: string; title: string }[];
}

export interface Worktree {
    bare: boolean;
    /** The short branch name; `null` when detached or bare. */
    branch: null | string;
    path: string;
    sha: null | string;
}
