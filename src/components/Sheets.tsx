import { type ReactNode, useState } from 'react';

import type { ResetMode } from '../lib/types';

import { branchLabel, isPushed, type Sheet, useRepo } from '../lib/store';
import { Button, Checkbox, Modal, TextArea, TextField } from './ui';

export function Sheets() {
    const sheet = useRepo((state) => state.sheet);
    const close = () => useRepo.getState().set({ sheet: null });

    if (!sheet) {
        return null;
    }

    return <SheetBody close={close} sheet={sheet} />;
}

function Footer({
    close,
    confirm,
    danger,
    disabled,
    onConfirm,
}: {
    close: () => void;
    confirm: string;
    danger?: boolean;
    disabled?: boolean;
    onConfirm: () => void;
}) {
    return (
        <div className="flex justify-end gap-2 pt-1">
            <Button onClick={close}>Cancel</Button>
            <Button
                disabled={disabled}
                onClick={() => {
                    onConfirm();
                    close();
                }}
                variant={danger ? 'danger' : 'primary'}
            >
                {confirm}
            </Button>
        </div>
    );
}

/** Submits on Enter (⌘Enter inside a textarea). */
function Form({
    children,
    disabled,
    onSubmit,
}: {
    children: ReactNode;
    disabled?: boolean;
    onSubmit: () => void;
}) {
    return (
        <form
            className="flex flex-col gap-3"
            onKeyDown={(event) => {
                if (event.key === 'Enter' && event.metaKey && !disabled) {
                    event.preventDefault();
                    onSubmit();
                }
            }}
            onSubmit={(event) => {
                event.preventDefault();

                if (!disabled) {
                    onSubmit();
                }
            }}
        >
            {children}
            <button hidden type="submit" />
        </form>
    );
}

const validRef = (name: string) => {
    const n = name.trim();

    return !!n && !/\s|\.\.|^-|\/$|\.lock$|[~^:?*[\\]/.test(n);
};

function joinMessage(summary: string, body: string) {
    const b = body.trim();

    return b ? `${summary.trim()}\n\n${b}` : summary.trim();
}

function MessageFields({
    body,
    setBody,
    setSummary,
    summary,
}: {
    body: string;
    setBody: (v: string) => void;
    setSummary: (v: string) => void;
    summary: string;
}) {
    return (
        <>
            <TextField
                aria-label="Summary"
                onChange={(event) => setSummary(event.target.value)}
                placeholder="Summary"
                value={summary}
            />
            <TextArea
                aria-label="Description"
                onChange={(event) => setBody(event.target.value)}
                placeholder="Description (optional)"
                rows={5}
                value={body}
            />
        </>
    );
}

function NewBranch({ close, from }: { close: () => void; from: null | string }) {
    const s = useRepo();
    const [name, setName] = useState('');
    const [checkout, setCheckout] = useState(true);

    const submit = () => {
        s.perform(
            { checkout, name: name.trim(), start: from, type: 'createBranch' },
            'Creating branch',
        );
        close();
    };

    const start = from
        ? /^[0-9a-f]{40}$/.test(from)
            ? from.slice(0, 7)
            : from
        : branchLabel(s.status);

    return (
        <Modal onClose={close} title="New branch">
            <Form disabled={!validRef(name)} onSubmit={submit}>
                <span className="text-muted">
                    Starts from <span className="font-mono text-soft">{start}</span>.
                </span>
                <TextField
                    aria-label="Branch name"
                    onChange={(event) => setName(event.target.value)}
                    placeholder="feature/my-change"
                    value={name}
                />
                <Checkbox
                    checked={checkout}
                    label="Switch to the new branch"
                    onChange={setCheckout}
                />
                <Footer
                    close={close}
                    confirm={checkout ? 'Create & switch' : 'Create'}
                    disabled={!validRef(name)}
                    onConfirm={submit}
                />
            </Form>
        </Modal>
    );
}

function RenameBranch({
    close,
    name,
    upstream,
}: {
    close: () => void;
    name: string;
    upstream: null | string;
}) {
    const s = useRepo();
    const [value, setValue] = useState(name);
    const ok = validRef(value) && value.trim() !== name;

    const submit = () => {
        s.perform({ new: value.trim(), old: name, type: 'renameBranch' }, 'Renaming branch');
        close();
    };

    return (
        <Modal onClose={close} title="Rename branch">
            <Form disabled={!ok} onSubmit={submit}>
                {upstream && (
                    <span className="text-muted">The remote branch {upstream} keeps its name.</span>
                )}
                <TextField
                    aria-label="Branch name"
                    onChange={(event) => setValue(event.target.value)}
                    value={value}
                />
                <Footer close={close} confirm="Rename" disabled={!ok} onConfirm={submit} />
            </Form>
        </Modal>
    );
}

function Reset({ close, sheet }: { close: () => void; sheet: Extract<Sheet, { kind: 'reset' }> }) {
    const s = useRepo();
    const [mode, setMode] = useState<ResetMode>('mixed');
    const options: [ResetMode, string, string][] = [
        ['soft', 'Soft', 'Keep the changes of later commits, staged.'],
        ['mixed', 'Mixed', 'Keep the changes of later commits, unstaged.'],
        [
            'hard',
            'Hard',
            'Throw away later commits and all local changes. ⌘Z still brings them back.',
        ],
    ];

    return (
        <Modal
            onClose={close}
            title={`Reset ${branchLabel(s.status)} to ${sheet.commit.sha.slice(0, 7)}?`}
            width={500}
        >
            <span className="text-muted">
                “{sheet.commit.subject}” becomes the latest commit on this branch.
            </span>
            <fieldset className="m-0 flex flex-col gap-2 border-0 p-0">
                <legend className="sr-only">Reset mode</legend>
                {options.map(([value, title, desc]) => (
                    <label
                        className={`flex cursor-pointer gap-2.5 rounded-lg border p-2.5 ${mode === value ? 'border-accent-line bg-accent-bg' : 'border-control-line'}`}
                        key={value}
                    >
                        <input
                            checked={mode === value}
                            className="m-0 mt-0.5 accent-[var(--accent)]"
                            name="mode"
                            onChange={() => setMode(value)}
                            type="radio"
                        />
                        <span className="flex flex-col">
                            <span className="font-semibold">{title}</span>
                            <span className="text-[12px] text-muted">{desc}</span>
                        </span>
                    </label>
                ))}
            </fieldset>
            <Footer
                close={close}
                confirm="Reset"
                danger={mode === 'hard'}
                onConfirm={() =>
                    s.perform({ mode, sha: sheet.commit.sha, type: 'reset' }, 'Resetting')
                }
            />
        </Modal>
    );
}

function Reword({
    close,
    sheet,
}: {
    close: () => void;
    sheet: Extract<Sheet, { kind: 'reword' }>;
}) {
    const s = useRepo();
    const [summary, setSummary] = useState(sheet.commit.subject);
    const [body, setBody] = useState(sheet.commit.body);
    const pushed = isPushed(s.meta, sheet.commit.sha);

    const submit = () => {
        s.perform(
            { message: joinMessage(summary, body), sha: sheet.commit.sha, type: 'reword' },
            'Renaming commit',
        );
        close();
    };

    return (
        <Modal onClose={close} title={`Rename commit ${sheet.commit.sha.slice(0, 7)}`} width={520}>
            <Form disabled={!summary.trim()} onSubmit={submit}>
                {pushed && (
                    <span className="text-warn">
                        Already pushed. Renaming rewrites shared history and needs a force push.
                    </span>
                )}
                <MessageFields
                    body={body}
                    setBody={setBody}
                    setSummary={setSummary}
                    summary={summary}
                />
                <Footer
                    close={close}
                    confirm="Rename"
                    disabled={!summary.trim()}
                    onConfirm={submit}
                />
            </Form>
        </Modal>
    );
}

function SheetBody({ close, sheet }: { close: () => void; sheet: Sheet }) {
    const s = useRepo();

    switch (sheet.kind) {
        case 'deleteBranch':
            return (
                <Modal onClose={close} title={`Delete branch ${sheet.name}?`}>
                    <p className="m-0 text-muted">
                        Commits only on this branch stay recoverable with ⌘Z.
                    </p>
                    <Footer
                        close={close}
                        confirm="Delete"
                        danger
                        onConfirm={() =>
                            s.perform(
                                { force: true, name: sheet.name, type: 'deleteBranch' },
                                'Deleting branch',
                            )
                        }
                    />
                </Modal>
            );
        case 'discard':
            return (
                <Modal
                    onClose={close}
                    title={
                        sheet.changes.length === 1
                            ? `Discard changes to ${sheet.changes[0].path}?`
                            : `Discard changes to ${sheet.changes.length} files?`
                    }
                >
                    <p className="m-0 text-muted">
                        New files are deleted and edits are thrown away. You can undo this with ⌘Z.
                    </p>
                    <Footer
                        close={close}
                        confirm="Discard"
                        danger
                        onConfirm={() =>
                            s.perform({ changes: sheet.changes, type: 'discard' }, 'Discarding')
                        }
                    />
                </Modal>
            );

        case 'drop': {
            const pushed = sheet.shas.some((sha) => isPushed(s.meta, sha));
            const subjects = s.commits
                .filter((commit) => sheet.shas.includes(commit.sha))
                .map((commit) => commit.subject);

            return (
                <Modal
                    onClose={close}
                    title={
                        sheet.shas.length === 1
                            ? 'Remove this commit?'
                            : `Remove ${sheet.shas.length} commits?`
                    }
                >
                    <p className={`m-0 ${pushed ? 'text-warn' : 'text-muted'}`}>
                        {pushed
                            ? 'At least one is already pushed. Removing rewrites shared history and needs a force push; Revert is safer.'
                            : 'Later commits are replayed without it. You can undo with ⌘Z.'}
                    </p>
                    <ul className="m-0 list-disc pl-5 text-soft">
                        {subjects.slice(0, 8).map((subject) => (
                            <li className="truncate" key={subject}>
                                {subject}
                            </li>
                        ))}
                        {subjects.length > 8 && (
                            <li className="text-muted">and {subjects.length - 8} more</li>
                        )}
                    </ul>
                    <Footer
                        close={close}
                        confirm="Remove"
                        danger
                        onConfirm={() => {
                            s.perform({ shas: sheet.shas, type: 'drop' }, 'Removing');
                            s.setChecked([]);
                        }}
                    />
                </Modal>
            );
        }

        case 'newBranch':
            return <NewBranch close={close} from={sheet.from} />;
        case 'renameBranch':
            return <RenameBranch close={close} name={sheet.name} upstream={sheet.upstream} />;
        case 'reset':
            return <Reset close={close} sheet={sheet} />;
        case 'reword':
            return <Reword close={close} sheet={sheet} />;
        case 'squash':
            return <Squash close={close} shas={sheet.shas} />;
        case 'stash':
            return <StashSheet close={close} paths={sheet.paths} />;
        case 'tag':
            return <TagSheet close={close} sha={sheet.commit.sha} subject={sheet.commit.subject} />;
    }
}

function Squash({ close, shas }: { close: () => void; shas: string[] }) {
    const s = useRepo();
    // Oldest commit's subject as the summary, the rest listed in the body.
    const selected = s.commits.filter((commit) => shas.includes(commit.sha)).reverse();
    const [summary, setSummary] = useState(selected[0]?.subject ?? '');
    const [body, setBody] = useState(
        selected
            .slice(1)
            .map((commit) => `* ${commit.subject}`)
            .join('\n'),
    );

    const submit = () => {
        s.perform({ message: joinMessage(summary, body), shas, type: 'squash' }, 'Squashing');
        s.setChecked([]);
        close();
    };

    return (
        <Modal onClose={close} title={`Squash ${shas.length} commits`} width={520}>
            <Form disabled={!summary.trim()} onSubmit={submit}>
                <span className="text-muted">
                    They become one commit in place of the oldest. Commits between them are kept.
                </span>
                <MessageFields
                    body={body}
                    setBody={setBody}
                    setSummary={setSummary}
                    summary={summary}
                />
                <Footer
                    close={close}
                    confirm="Squash"
                    disabled={!summary.trim()}
                    onConfirm={submit}
                />
            </Form>
        </Modal>
    );
}

function StashSheet({ close, paths }: { close: () => void; paths: string[] }) {
    const s = useRepo();
    const [message, setMessage] = useState('');
    const [untracked, setUntracked] = useState(true);

    const submit = () => {
        s.perform(
            {
                includeUntracked: untracked,
                message: message.trim() || null,
                paths,
                type: 'stashPush',
            },
            'Stashing',
        );
        close();
    };

    const title =
        paths.length === 0
            ? 'Stash changes'
            : paths.length === 1
              ? `Stash ${paths[0].split('/').pop()}`
              : `Stash ${paths.length} files`;

    return (
        <Modal onClose={close} title={title}>
            <Form onSubmit={submit}>
                <span className="text-muted">
                    Puts the changes aside and leaves a clean working copy. Bring them back from
                    History › Stashes.
                </span>
                <TextField
                    aria-label="Message"
                    onChange={(event) => setMessage(event.target.value)}
                    placeholder="What were you trying?"
                    value={message}
                />
                <Checkbox
                    checked={untracked}
                    label="Include untracked files"
                    onChange={setUntracked}
                />
                <Footer close={close} confirm="Stash" onConfirm={submit} />
            </Form>
        </Modal>
    );
}

function TagSheet({ close, sha, subject }: { close: () => void; sha: string; subject: string }) {
    const s = useRepo();
    const [name, setName] = useState('');
    const [message, setMessage] = useState('');

    const submit = () => {
        s.perform(
            { message: message.trim() || null, name: name.trim(), sha, type: 'createTag' },
            'Tagging',
        );
        close();
    };

    return (
        <Modal onClose={close} title={`Tag ${sha.slice(0, 7)}`}>
            <Form disabled={!validRef(name)} onSubmit={submit}>
                <span className="truncate text-muted">{subject}</span>
                <TextField
                    aria-label="Tag name"
                    onChange={(event) => setName(event.target.value)}
                    placeholder="v1.0.0"
                    value={name}
                />
                <TextField
                    aria-label="Message"
                    onChange={(event) => setMessage(event.target.value)}
                    placeholder="Message (makes an annotated tag)"
                    value={message}
                />
                <Footer
                    close={close}
                    confirm="Create tag"
                    disabled={!validRef(name)}
                    onConfirm={submit}
                />
            </Form>
        </Modal>
    );
}
