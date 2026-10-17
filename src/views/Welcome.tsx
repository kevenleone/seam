import { listen } from '@tauri-apps/api/event';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { open } from '@tauri-apps/plugin-dialog';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { useEffect, useState } from 'react';

import { Icon, SeamMark } from '../components/Icon';
import { Button, Modal, TextField } from '../components/ui';
import { api, errorMessage } from '../lib/api';
import { recentRepositories, removeRecent, syncRecents } from '../lib/settings';
import { useAppVersion } from '../lib/version';

const home = (path: string) => path.replace(/^\/Users\/[^/]+/, '~');

export function Welcome() {
    const version = useAppVersion();
    const [recents, setRecents] = useState(recentRepositories);
    const [error, setError] = useState<null | string>(null);
    const [cloning, setCloning] = useState(false);
    const [dropping, setDropping] = useState(false);

    useEffect(() => {
        void syncRecents().then(setRecents);

        const win = getCurrentWebviewWindow();
        const offs = [
            win.listen<string>('menu', (event) => event.payload === 'clone' && setCloning(true)),
            listen('recent-opened', () => void syncRecents().then(setRecents)),
            // Folders dropped on the window.
            win.onDragDropEvent((event) => {
                if (event.payload.type === 'over' || event.payload.type === 'enter') {
                    setDropping(true);
                } else if (event.payload.type === 'leave') {
                    setDropping(false);
                } else if (event.payload.type === 'drop') {
                    setDropping(false);
                    event.payload.paths.forEach((path) => void openPath(path));
                }
            }),
        ];

        return () => offs.forEach((promise) => promise.then((stop) => stop()));
    }, []);

    async function openPath(path: string) {
        setError(null);

        try {
            if (!(await api.topLevel(path))) {
                if (
                    !window.confirm(
                        `“${path.split('/').pop()}” isn’t a git repository. Create one here?`,
                    )
                ) {
                    return;
                }

                await api.initRepository(path);
            }

            await api.openRepository(path);
        } catch (e) {
            setError(errorMessage(e));
        }
    }

    async function choose() {
        const picked = await open({ directory: true, title: 'Open a git repository' });

        if (typeof picked === 'string') {
            void openPath(picked);
        }
    }

    return (
        <div className="flex h-full bg-app text-fg">
            <div
                className="flex w-[430px] shrink-0 flex-col gap-6 px-9 pt-14 pb-7"
                data-tauri-drag-region
            >
                <div className="flex items-center gap-4" data-tauri-drag-region>
                    <SeamMark size={64} />
                    <div className="flex flex-col">
                        <span className="text-[30px] font-semibold tracking-tight">Seam</span>
                        <span className="text-muted">Version {version}</span>
                    </div>
                </div>
                <ul className="m-0 flex list-none flex-col gap-4 p-0">
                    {[
                        [
                            'Every diff in one place',
                            'See what changed in a repository, a branch or a commit range, hunk by hunk.',
                        ],
                        [
                            'Stage exactly what you mean',
                            'Stage, unstage or discard single lines and hunks, then commit or amend.',
                        ],
                        [
                            'Everyday git, undoable',
                            'Stash, rename or remove commits, reset or revert, and undo any of it with ⌘Z.',
                        ],
                    ].map(([title, text]) => (
                        <li className="flex flex-col gap-0.5" key={title}>
                            <span className="font-semibold">{title}</span>
                            <span className="leading-normal text-soft">{text}</span>
                        </li>
                    ))}
                </ul>
                <span className="flex-1" />
                <button
                    className="flex items-center gap-1.5 self-start text-[12px] text-muted hover:text-fg"
                    onClick={() => void api.openSettingsWindow(null)}
                    type="button"
                >
                    <Icon name="gear" size={12} /> Settings
                    <span className="font-mono text-[11px] text-dim">⌘,</span>
                </button>
            </div>
            <div
                className={`flex min-w-0 flex-1 flex-col gap-2.5 overflow-hidden border-l border-line p-6 pt-12 ${dropping ? 'bg-accent-bg' : 'bg-panel'}`}
            >
                <Button className="justify-start" onClick={() => void choose()} size="lg">
                    <Icon name="folder" /> Open repository… <span className="flex-1" />
                    <span className="font-mono text-[11px] text-muted">⌘O</span>
                </Button>
                <Button className="justify-start" onClick={() => setCloning(true)} size="lg">
                    <Icon name="download" /> Clone repository…
                </Button>
                <span className="mt-3 text-[11px] font-semibold tracking-[0.08em] text-muted uppercase">
                    Recent
                </span>
                {recents.length === 0 ? (
                    <span className="text-muted">
                        Repositories you open appear here. You can also drop a folder on this
                        window.
                    </span>
                ) : (
                    <ul className="m-0 flex min-h-0 min-w-0 list-none flex-col gap-0.5 overflow-auto p-0">
                        {recents.map((path) => (
                            <li
                                className="group flex min-w-0 items-center rounded-md hover:bg-hover"
                                key={path}
                            >
                                <button
                                    className="flex min-w-0 flex-1 flex-col px-2.5 py-1.5 text-left"
                                    onClick={() => void openPath(path)}
                                    type="button"
                                >
                                    <span className="font-medium">{path.split('/').pop()}</span>
                                    <span className="truncate text-[12px] text-muted">
                                        {home(path.split('/').slice(0, -1).join('/'))}
                                    </span>
                                </button>
                                <button
                                    aria-label="Show in Finder"
                                    className="hidden px-1.5 text-muted group-hover:block hover:text-fg"
                                    onClick={() => void revealItemInDir(path)}
                                    title="Show in Finder"
                                    type="button"
                                >
                                    <Icon name="external" size={12} />
                                </button>
                                <button
                                    aria-label="Remove from list"
                                    className="hidden px-2 text-muted group-hover:block hover:text-fg"
                                    onClick={() => {
                                        void removeRecent(path).then(setRecents);
                                    }}
                                    title="Remove from list"
                                    type="button"
                                >
                                    <Icon name="x" size={12} />
                                </button>
                            </li>
                        ))}
                    </ul>
                )}
                {error && (
                    <div className="rounded-md border border-del-line bg-del-bg p-2.5 text-body">
                        {error}
                    </div>
                )}
            </div>
            {cloning && <CloneSheet close={() => setCloning(false)} />}
        </div>
    );
}

function CloneSheet({ close }: { close: () => void }) {
    const [url, setUrl] = useState('');
    const [parent, setParent] = useState<null | string>(null);
    const [progress, setProgress] = useState<null | string>(null);
    const [error, setError] = useState<null | string>(null);
    const name =
        url
            .trim()
            .split('/')
            .filter(Boolean)
            .pop()
            ?.replace(/\.git$/, '') ?? '';

    useEffect(() => {
        const off = getCurrentWebviewWindow().listen<string>('git-progress', (event) =>
            setProgress(event.payload),
        );

        return () => void off.then((unlisten) => unlisten());
    }, []);

    async function choose() {
        const picked = await open({ directory: true, title: 'Clone into' });

        if (typeof picked === 'string') {
            setParent(picked);
        }
    }

    async function clone() {
        if (!parent || !name) {
            return;
        }

        setError(null);
        setProgress('Starting…');

        try {
            await api.cloneRepository(url.trim(), `${parent}/${name}`);
        } catch (e) {
            setError(errorMessage(e));
            setProgress(null);
        }
    }

    return (
        <Modal onClose={close} title="Clone repository" width={480}>
            <TextField
                aria-label="Repository URL"
                onChange={(event) => setUrl(event.target.value)}
                placeholder="git@github.com:owner/repo.git"
                value={url}
            />
            <div className="flex items-center gap-2">
                <span className="min-w-0 flex-1 truncate text-muted">
                    {parent ? `${home(parent)}/${name}` : 'Choose where to put it'}
                </span>
                <Button onClick={() => void choose()} size="sm">
                    Choose…
                </Button>
            </div>
            {progress && <span className="truncate text-[12px] text-muted">{progress}</span>}
            {error && <span className="selectable text-del">{error}</span>}
            <div className="flex justify-end gap-2">
                <Button onClick={close}>Cancel</Button>
                <Button
                    disabled={!name || !parent || (!!progress && !error)}
                    onClick={() => void clone()}
                    variant="primary"
                >
                    Clone
                </Button>
            </div>
        </Modal>
    );
}
