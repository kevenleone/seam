import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { useEffect, useMemo, useState } from 'react';

import type { Branch, Worktree } from '../lib/types';

import { api, errorMessage } from '../lib/api';
import { recentRepositories } from '../lib/settings';
import { type Picker, repoName, useRepo } from '../lib/store';
import { type PickItem, QuickPick } from './QuickPick';
import { relativeTime } from './ui';

type BranchMode = Extract<Picker, 'branch' | 'branchDetached' | 'branchFrom'>;

const PLACEHOLDERS: Record<BranchMode, string> = {
    branch: 'Select a branch or tag to check out',
    branchDetached: 'Select a branch or tag to check out detached',
    branchFrom: 'Select a ref to create the branch from',
};

interface Ref {
    branch?: Branch;
    kind: 'branch' | 'remote' | 'tag';
    name: string;
}

/** Every branch and tag, plus creating one; after VS Code's "Checkout to…". */
export function BranchPicker({ mode }: { mode: BranchMode }) {
    const s = useRepo();
    const close = () => s.set({ picker: null });
    const items = useMemo(() => {
        const refs = refItems(s.meta.branches, s.meta.tags, (ref) => {
            if (mode === 'branchFrom') {
                s.set({ sheet: { from: ref.name, kind: 'newBranch' } });
            } else if (mode === 'branchDetached' || ref.kind === 'tag') {
                s.perform({ rev: ref.name, type: 'checkoutDetached' }, 'Checking out');
            } else if (ref.branch?.worktree) {
                s.openWorktree(ref.branch.worktree);
            } else if (!ref.branch?.isCurrent) {
                s.perform({ branch: ref.name, type: 'checkout' }, 'Switching branch');
            }
        });

        if (mode !== 'branch') {
            return refs;
        }

        const actions: PickItem[] = [
            {
                group: 'actions',
                icon: 'plus',
                id: 'create',
                label: 'Create new branch…',
                run: () => s.set({ sheet: { from: null, kind: 'newBranch' } }),
            },
            {
                group: 'actions',
                icon: 'plus',
                id: 'create-from',
                label: 'Create new branch from…',
                run: () => s.set({ picker: 'branchFrom' }),
            },
            {
                group: 'actions',
                icon: 'commit',
                id: 'detached',
                label: 'Check out detached…',
                run: () => s.set({ picker: 'branchDetached' }),
            },
        ];

        return [...actions, ...refs];
    }, [mode, s.meta]);

    return <QuickPick items={items} onClose={close} placeholder={PLACEHOLDERS[mode]} />;
}

/** This repository's worktrees and the recent repositories, plus opening another folder. */
export function WorkspacePicker() {
    const s = useRepo();
    const [worktrees, setWorktrees] = useState<Worktree[]>([]);
    const close = () => s.set({ picker: null });

    useEffect(() => {
        api.worktrees(s.root).then(
            (list) => setWorktrees(list.filter((worktree) => !worktree.bare)),
            () => setWorktrees([]),
        );
    }, [s.root]);

    const items = useMemo(() => {
        const open = (path: string) =>
            api.openRepository(path).catch((error: unknown) => s.notify(errorMessage(error)));
        const paths = new Set(worktrees.map((worktree) => worktree.path));
        const list: PickItem[] = [
            {
                group: 'actions',
                icon: 'folder',
                id: 'open',
                label: 'Open folder…',
                run: () => void api.openFolderDialog(),
            },
            {
                group: 'actions',
                icon: 'external',
                id: 'reveal',
                label: 'Reveal in Finder',
                run: () => void revealItemInDir(s.root),
            },
        ];

        if (worktrees.length > 1) {
            for (const worktree of worktrees) {
                const current = worktree.path === s.root;

                list.push({
                    description: current ? 'current' : undefined,
                    detail: worktree.path,
                    group: 'worktrees',
                    icon: 'branch',
                    id: `worktree:${worktree.path}`,
                    label: `${repoName(worktree.path)} · ${worktree.branch ?? `detached at ${worktree.sha?.slice(0, 7) ?? '?'}`}`,
                    run: () => !current && s.openWorktree(worktree.path),
                });
            }
        }

        for (const path of recentRepositories()) {
            if (path === s.root || paths.has(path)) {
                continue;
            }

            list.push({
                detail: path,
                group: 'recent',
                icon: 'folder',
                id: `recent:${path}`,
                label: repoName(path),
                run: () => void open(path),
            });
        }

        return list;
    }, [s.root, worktrees]);

    return <QuickPick items={items} onClose={close} placeholder="Select a workspace to open" />;
}

function refItems(
    branches: Branch[],
    tags: { name: string; sha: string }[],
    pick: (ref: Ref) => void,
): PickItem[] {
    const branch = (entry: Branch): PickItem => {
        const counts = [
            entry.behind > 0 && `↓${entry.behind}`,
            entry.ahead > 0 && `↑${entry.ahead}`,
        ].filter(Boolean);
        const description = [
            entry.isCurrent && 'current',
            entry.worktree && 'worktree',
            counts.join(' '),
            entry.date > 0 && relativeTime(entry.date),
        ]
            .filter(Boolean)
            .join(' • ');

        return {
            description,
            detail: entry.author ? `${entry.author} • ${entry.subject}` : entry.subject,
            group: entry.isRemote ? 'remote branches' : 'branches',
            icon: 'branch',
            id: entry.fullName,
            label: entry.name,
            run: () =>
                pick({
                    branch: entry,
                    kind: entry.isRemote ? 'remote' : 'branch',
                    name: entry.name,
                }),
            terms: description,
        };
    };

    return [
        ...branches.filter((entry) => !entry.isRemote).map(branch),
        ...branches.filter((entry) => entry.isRemote).map(branch),
        ...tags.map((tag): PickItem => ({
            description: tag.sha.slice(0, 7),
            group: 'tags',
            icon: 'tag',
            id: `refs/tags/${tag.name}`,
            label: tag.name,
            run: () => pick({ kind: 'tag', name: tag.name }),
        })),
    ];
}
