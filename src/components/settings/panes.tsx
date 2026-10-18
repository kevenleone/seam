import { openUrl } from '@tauri-apps/plugin-opener';
import { useEffect, useState } from 'react';

import type { Mode } from '../../lib/themes';
import type {
    AiConfig,
    CliStatus,
    NotifyPrefs,
    Provider,
    ProviderStatus,
    ReviewFocusArea,
    ReviewStyle,
} from '../../lib/types';

import { PROVIDER_LABELS, REVIEW_FOCUS, useAi } from '../../lib/ai';
import { api, errorMessage } from '../../lib/api';
import { REPOSITORY_URL } from '../../lib/links';
import {
    effectiveMode,
    syncRecents,
    type Theme,
    updateSettings,
    useSettings,
} from '../../lib/settings';
import { ACCENTS, themesFor } from '../../lib/themes';
import { useAppVersion } from '../../lib/version';
import { SeamMark } from '../Icon';
import { Button } from '../ui';
import { Row, SectionLabel, Segmented, SELECT, TEXT_FIELD, Toggle } from './controls';
import { ThemePreview } from './ThemePreview';

// MARK: General

export function GeneralPane() {
    const settings = useSettings();
    const [gitPath, setGitPath] = useState(settings.gitPath);
    const [version, setVersion] = useState('');
    const [recents, setRecents] = useState<null | number>(null);

    useEffect(() => {
        void api
            .setGitPath(settings.gitPath || null)
            .then(() =>
                api.gitVersion().then(setVersion, () => setVersion('Git not found at this path')),
            );
    }, [settings.gitPath]);

    useEffect(() => {
        void syncRecents().then((list) => setRecents(list.length));
    }, []);

    return (
        <>
            <SectionLabel first>Git</SectionLabel>
            <Row
                desc={
                    <>
                        Seam runs this git for every action, with your config, hooks and credential
                        helpers. <span className="font-mono">{version}</span>
                    </>
                }
                title="Git executable"
            >
                <input
                    aria-label="Git executable"
                    className={TEXT_FIELD}
                    onBlur={() => updateSettings({ gitPath: gitPath.trim() })}
                    onChange={(event) => setGitPath(event.target.value)}
                    onKeyDown={(event) =>
                        event.key === 'Enter' && updateSettings({ gitPath: gitPath.trim() })
                    }
                    placeholder="/usr/bin/git"
                    spellCheck={false}
                    value={gitPath}
                />
            </Row>
            <Row
                desc="What Pull does when your branch and its upstream have both moved."
                last
                title="When pulling"
            >
                <Segmented
                    label="When pulling"
                    onChange={(pullMode) => updateSettings({ pullMode })}
                    options={[
                        { label: 'Fast-forward', value: 'fastForwardOnly' },
                        { label: 'Rebase', value: 'rebase' },
                        { label: 'Merge', value: 'merge' },
                    ]}
                    value={settings.pullMode}
                />
            </Row>

            <SectionLabel>Diffs</SectionLabel>
            <Row desc="Also switchable above every diff." title="Layout">
                <Segmented
                    label="Diff layout"
                    onChange={(value) => updateSettings({ splitDiff: value === 'split' })}
                    options={[
                        { label: 'Unified', value: 'unified' },
                        { label: 'Split', value: 'split' },
                    ]}
                    value={settings.splitDiff ? 'split' : 'unified'}
                />
            </Row>
            <Row
                desc="Colours code in diffs: TypeScript, JavaScript, Rust, Python, Java, Go, Markdown, JSON, YAML, CSS, HTML, shell, SQL, C, C++, C#, Kotlin, Swift, Ruby, PHP and more. Each theme brings its own colours."
                title="Syntax highlighting"
            >
                <Toggle
                    label="Syntax highlighting"
                    on={settings.syntaxHighlight}
                    onChange={(syntaxHighlight) => updateSettings({ syntaxHighlight })}
                />
            </Row>
            <Row
                desc="Lines that only change indentation or spacing are left out."
                last
                title="Ignore whitespace"
            >
                <Toggle
                    label="Ignore whitespace"
                    on={settings.ignoreWhitespace}
                    onChange={(ignoreWhitespace) => updateSettings({ ignoreWhitespace })}
                />
            </Row>

            <SectionLabel>Worktrees</SectionLabel>
            <Row
                desc="Opening a branch's worktree, or going back to the checkout it belongs to, closes the window you came from, so one window follows you around."
                last
                title="Replace the window"
            >
                <Toggle
                    label="Replace the window"
                    on={settings.worktreeReplacesWindow}
                    onChange={(worktreeReplacesWindow) =>
                        updateSettings({ worktreeReplacesWindow })
                    }
                />
            </Row>

            <NotificationsSection />

            <SectionLabel>Menu bar</SectionLabel>
            <Row
                desc="Closing the last window leaves Seam in the menu bar, with your recent repositories and a review of the current branch one click away. Quit from there or with ⌘Q."
                title="Keep Seam in the menu bar"
            />
            <Row
                desc={
                    recents === null
                        ? ' '
                        : `${recents} repositor${recents === 1 ? 'y' : 'ies'} in the welcome window and the menu bar.`
                }
                last
                title="Recent repositories"
            >
                <Button
                    disabled={!recents}
                    onClick={async () => {
                        for (const path of await syncRecents()) {
                            await api.removeRecentRepository(path);
                        }

                        setRecents((await syncRecents()).length);
                    }}
                    size="sm"
                >
                    Clear list
                </Button>
            </Row>

            <TerminalSection />
        </>
    );
}

function NotificationsSection() {
    const [prefs, setPrefs] = useState<NotifyPrefs | null>(null);
    const [sounds, setSounds] = useState<string[]>([]);

    useEffect(() => {
        void api.notifyPrefs().then(setPrefs);
        void api.sounds().then(setSounds);
    }, []);

    if (!prefs) {
        return null;
    }

    const save = (patch: Partial<NotifyPrefs>) => {
        const next = { ...prefs, ...patch };

        setPrefs(next);
        void api.setNotifyPrefs(next);
    };

    return (
        <>
            <SectionLabel>When an AI run finishes</SectionLabel>
            <Row
                desc="A notification when a review, recap or plan is ready or has failed, shown while its window isn't in front. Seam keeps working after you close the window."
                title="Notify me"
            >
                <Toggle
                    label="Notify me"
                    on={prefs.banner}
                    onChange={(banner) => save({ banner })}
                />
            </Row>
            <Row desc="Plays whether or not Seam is in front." last title="Sound">
                <select
                    aria-label="Sound"
                    className={SELECT}
                    onChange={(event) => {
                        save({ sound: event.target.value });
                        void api.playSound(event.target.value);
                    }}
                    value={prefs.sound}
                >
                    <option value="">None</option>
                    {sounds.map((sound) => (
                        <option key={sound} value={sound}>
                            {sound}
                        </option>
                    ))}
                </select>
                <Button
                    disabled={!prefs.sound}
                    onClick={() => void api.playSound(prefs.sound)}
                    size="sm"
                >
                    Play
                </Button>
            </Row>
        </>
    );
}

function TerminalSection() {
    const [status, setStatus] = useState<CliStatus | null>(null);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<null | string>(null);

    useEffect(() => {
        void api.cliStatus().then(setStatus, () => setStatus(null));
    }, []);

    if (!status) {
        return null;
    }

    const title = `Install '${status.name}' command in PATH`;

    const toggle = async (): Promise<void> => {
        if (busy) {
            return;
        }

        setBusy(true);
        setError(null);

        try {
            setStatus(await api.setCliInstalled(!status.installed));
        } catch (reason) {
            setError(errorMessage(reason));
        } finally {
            setBusy(false);
        }
    };

    return (
        <>
            <SectionLabel>Terminal</SectionLabel>
            <Row
                desc={
                    <>
                        Run <span className="font-mono">{status.name} .</span> to open the
                        repository you are in. Links /usr/local/bin/{status.name}; macOS may ask for
                        your password.
                    </>
                }
                last
                title={title}
            >
                <Toggle label={title} on={status.installed} onChange={() => void toggle()} />
            </Row>
            {error && (
                <div
                    className="pb-3.5 text-[12.5px] leading-normal whitespace-pre-wrap text-warn select-text"
                    role="alert"
                >
                    {error}
                </div>
            )}
        </>
    );
}

// MARK: Appearance

const MODES: { id: Theme; label: string; swatch: string }[] = [
    { id: 'light', label: 'Light', swatch: '#f4f3ef' },
    { id: 'dark', label: 'Dark', swatch: '#1d1f26' },
    { id: 'auto', label: 'Auto', swatch: 'linear-gradient(135deg, #f4f3ef 50%, #1d1f26 50%)' },
];

export function AppearancePane() {
    const settings = useSettings();
    const mode: Mode = effectiveMode(settings.theme);
    const chosen = mode === 'dark' ? settings.darkTheme : settings.lightTheme;
    const accentName = ACCENTS.find((accent) => accent.id === settings.accent)?.name ?? '';

    return (
        <>
            <SectionLabel first>Color mode</SectionLabel>
            <div className="flex gap-2.5">
                {MODES.map((option) => {
                    const active = settings.theme === option.id;

                    return (
                        <button
                            aria-pressed={active}
                            className={`flex flex-1 flex-col gap-2 rounded-xl border-[1.5px] p-2.5 ${active ? 'border-accent bg-accent-bg' : 'border-line hover:border-control-line'}`}
                            key={option.id}
                            onClick={() => updateSettings({ theme: option.id })}
                            type="button"
                        >
                            {/* A literal colour: the swatch previews the mode itself. */}
                            <span
                                className="h-[52px] rounded-lg border border-line"
                                style={{ background: option.swatch }}
                            />
                            <span className={active ? 'font-semibold' : 'text-soft'}>
                                {option.label}
                            </span>
                        </button>
                    );
                })}
            </div>

            <SectionLabel>Theme style</SectionLabel>
            <p className="m-0 mb-2.5 text-[12.5px] text-muted">
                The colours used throughout Seam. Light and dark keep separate picks; additions stay
                blue and deletions orange in every theme.
            </p>
            <div className="grid grid-cols-4 gap-2.5">
                {themesFor(mode).map((theme) => {
                    const active = chosen === theme.id;

                    return (
                        <button
                            aria-pressed={active}
                            className={`flex flex-col gap-1.5 rounded-xl border-[1.5px] p-1.5 ${active ? 'border-accent bg-accent-bg' : 'border-line hover:border-control-line'}`}
                            key={theme.id}
                            onClick={() =>
                                updateSettings(
                                    mode === 'dark'
                                        ? { darkTheme: theme.id }
                                        : { lightTheme: theme.id },
                                )
                            }
                            type="button"
                        >
                            <ThemePreview theme={theme} />
                            <span
                                className={`truncate text-[12px] ${active ? 'font-semibold' : 'text-soft'}`}
                            >
                                {theme.name}
                            </span>
                        </button>
                    );
                })}
            </div>

            <SectionLabel>Accent</SectionLabel>
            <div className="flex items-center gap-2.5">
                {ACCENTS.map((accent) => {
                    const active = settings.accent === accent.id;
                    const color = mode === 'dark' ? accent.dark : accent.light;

                    return (
                        <button
                            aria-label={accent.name}
                            aria-pressed={active}
                            className="flex h-7 w-7 items-center justify-center rounded-full border-[1.5px]"
                            key={accent.id}
                            onClick={() => updateSettings({ accent: accent.id })}
                            // The ring and the dot are the colour being offered.
                            style={{ borderColor: active ? color : 'transparent' }}
                            title={accent.name}
                            type="button"
                        >
                            <span className="h-5 w-5 rounded-full" style={{ background: color }} />
                        </button>
                    );
                })}
                <span className="ml-1 text-muted">{accentName}</span>
            </div>

            <SectionLabel>Text &amp; layout</SectionLabel>
            <Row title="Code size">
                <span className="text-[11px] text-muted">A</span>
                <input
                    aria-label="Code size"
                    className="w-[140px] accent-[var(--accent)]"
                    max={16}
                    min={10.5}
                    onChange={(event) => updateSettings({ codeSize: Number(event.target.value) })}
                    step={0.5}
                    type="range"
                    value={settings.codeSize}
                />
                <span className="text-[16px] text-muted">A</span>
            </Row>
            <Row
                desc="Repository, branch, sync state and progress along the bottom of the window."
                title="Show the status bar"
            >
                <Toggle
                    label="Show the status bar"
                    on={settings.statusBar}
                    onChange={(statusBar) => updateSettings({ statusBar })}
                />
            </Row>
            <Row
                desc="Panels and the diff expander snap into place instead of sliding."
                last
                title="Reduce motion"
            >
                <Toggle
                    label="Reduce motion"
                    on={settings.reduceMotion}
                    onChange={(reduceMotion) => updateSettings({ reduceMotion })}
                />
            </Row>
        </>
    );
}

// MARK: AI

const LANGUAGES = [
    'English',
    'Português (Brasil)',
    'Español',
    'Français',
    'Deutsch',
    'Italiano',
    '日本語',
    '中文',
];

export function AiPane() {
    const { config, providers, refresh, save } = useAi();

    if (!config) {
        return <p className="text-muted">Loading…</p>;
    }

    const current = providers.find((provider) => provider.id === config.provider);
    const harnesses = providers.filter((provider) => provider.kind === 'harness');
    const apis = providers.filter((provider) => provider.kind === 'api');
    const label = (provider: ProviderStatus) =>
        provider.kind === 'harness' && !provider.available
            ? `${provider.label} (not installed)`
            : provider.label;

    return (
        <>
            <p className="m-0 text-[12.5px] leading-normal text-muted">
                Seam sends nothing to a model until you start a review. Pick the agent you already
                use, or a model provider and its key; there is no Seam account in between.
            </p>

            <SectionLabel>Assistant</SectionLabel>
            <Row desc={privacyNote(current)} title="Model">
                <select
                    aria-label="Model"
                    className={SELECT}
                    onChange={(event) =>
                        void save({ model: '', provider: event.target.value as Provider })
                    }
                    value={config.provider}
                >
                    <optgroup label="Coding agents on this Mac">
                        {harnesses.map((provider) => (
                            <option
                                disabled={!provider.available && provider.id !== config.provider}
                                key={provider.id}
                                value={provider.id}
                            >
                                {label(provider)}
                            </option>
                        ))}
                    </optgroup>
                    <optgroup label="Model providers">
                        {apis.map((provider) => (
                            <option key={provider.id} value={provider.id}>
                                {provider.label}
                            </option>
                        ))}
                    </optgroup>
                </select>
            </Row>
            {current?.kind === 'harness' && current.path && (
                <Row desc={<span className="font-mono">{current.path}</span>} title="Found at" />
            )}
            {config.provider === 'openai-compatible' && (
                <TextSetting
                    desc="Where the endpoint listens, up to and including /v1. Ollama's is http://localhost:11434/v1, LM Studio's http://localhost:1234/v1."
                    onCommit={(baseUrl) => save({ baseUrl })}
                    placeholder="http://localhost:11434/v1"
                    title="Endpoint URL"
                    value={config.baseUrl}
                />
            )}
            {current?.kind === 'api' && (
                <KeyRow
                    onChange={refresh}
                    optional={current.id === 'openai-compatible'}
                    provider={current}
                />
            )}
            {current && (
                <TextSetting
                    desc={
                        current.kind === 'harness'
                            ? `Leave empty to use ${current.label}'s own default.${current.id === 'opencode' ? ' OpenCode names models as provider/model.' : ''}`
                            : current.defaultModel
                              ? `Leave empty to use ${current.defaultModel}.`
                              : "The model's id, exactly as the provider names it."
                    }
                    onCommit={(model) => save({ model })}
                    placeholder={
                        current.defaultModel ??
                        (current.kind === 'harness' ? 'Default' : 'Model id')
                    }
                    title="Model name"
                    value={config.model}
                />
            )}
            <TestRow config={config} />

            <SectionLabel>Reviews</SectionLabel>
            <ReviewStyleRows review={config.review} save={(review) => save({ review })} />
        </>
    );
}

function KeyRow({
    onChange,
    optional,
    provider,
}: {
    onChange(): void;
    optional: boolean;
    provider: ProviderStatus;
}) {
    const [draft, setDraft] = useState('');
    const [failure, setFailure] = useState<null | string>(null);

    const run = (work: Promise<void>) => {
        setFailure(null);
        work.then(() => {
            setDraft('');
            onChange();
        }).catch((error: unknown) => setFailure(errorMessage(error)));
    };

    const save = () => draft.trim() && run(api.setAiKey(provider.id, draft.trim()));

    return (
        <Row
            desc={
                failure ??
                (optional
                    ? 'Only if the endpoint asks for one, and only ever sent to it. Kept in the macOS Keychain.'
                    : 'Kept in the macOS Keychain on this Mac, sent only to the provider, and never shown again.')
            }
            title="API key"
        >
            <div className="flex flex-col items-end gap-1.5">
                <input
                    aria-label={`${provider.label} API key`}
                    className={TEXT_FIELD}
                    onBlur={save}
                    onChange={(event) => setDraft(event.target.value)}
                    onKeyDown={(event) => event.key === 'Enter' && save()}
                    placeholder={provider.hasKey ? 'Saved. Paste to replace' : 'Paste a key'}
                    spellCheck={false}
                    type="password"
                    value={draft}
                />
                {provider.hasKey && (
                    <button
                        className="text-[12px] text-muted underline hover:text-fg"
                        onClick={() => run(api.clearAiKey(provider.id))}
                        type="button"
                    >
                        Remove key
                    </button>
                )}
            </div>
        </Row>
    );
}

function privacyNote(provider: ProviderStatus | undefined): string {
    if (!provider) {
        return '';
    }

    if (provider.kind === 'harness') {
        return `Runs ${provider.label} on this Mac with its own sign-in, read-only in the repository. The diff, and any files it opens to understand it, go to whichever provider ${provider.label} is set up with.`;
    }

    if (provider.id === 'openai-compatible') {
        return 'The diff goes to the endpoint below. Point it at a model running on this Mac (Ollama, LM Studio) and nothing leaves it.';
    }

    return `The diff goes to ${provider.label} under your own API key. The model sees only the diff, not the rest of the repository.`;
}

function ReviewStyleRows({
    review,
    save,
}: {
    review: ReviewStyle;
    save(review: ReviewStyle): void;
}) {
    const toggle = (id: ReviewFocusArea) =>
        save({
            ...review,
            focus: review.focus.includes(id)
                ? review.focus.filter((focus) => focus !== id)
                : [...review.focus, id],
        });

    return (
        <>
            <Row
                desc="Your default for every repository. The agents also read a repository's AGENTS.md and CLAUDE.md."
                title="Look for"
            />
            <div className="grid grid-cols-2 gap-2 pb-3.5">
                {REVIEW_FOCUS.map((focus) => {
                    const on = review.focus.includes(focus.id);

                    return (
                        <button
                            aria-pressed={on}
                            className={`flex items-start gap-2.5 rounded-lg border p-2.5 text-left ${on ? 'border-accent-line bg-accent-bg' : 'border-line hover:border-control-line'}`}
                            key={focus.id}
                            onClick={() => toggle(focus.id)}
                            type="button"
                        >
                            <span
                                className={`mt-0.5 flex h-4 w-4 flex-none items-center justify-center rounded border text-[11px] ${on ? 'border-accent bg-accent text-on-accent' : 'border-control-line'}`}
                            >
                                {on ? '✓' : ''}
                            </span>
                            <span className="flex flex-col">
                                <span className="font-semibold">{focus.title}</span>
                                <span className="text-[12px] text-muted">{focus.desc}</span>
                            </span>
                        </button>
                    );
                })}
            </div>
            <Row
                desc={
                    {
                        balanced:
                            'Risks and checks worth a look, plus short notes on behaviour changes. No style nitpicks.',
                        blockers:
                            'Only things that should stop a merge. Expect zero to two comments on most diffs.',
                        thorough:
                            'Everything above, plus suggestions and readability notes. Best before a big release.',
                    }[review.level]
                }
                title="How picky"
            >
                <Segmented
                    label="How picky"
                    onChange={(level) => save({ ...review, level })}
                    options={[
                        { label: 'Blockers only', value: 'blockers' },
                        { label: 'Balanced', value: 'balanced' },
                        { label: 'Thorough', value: 'thorough' },
                    ]}
                    value={review.level}
                />
            </Row>
            <Row last title="Write comments in">
                <select
                    aria-label="Comment language"
                    className={SELECT}
                    onChange={(event) => save({ ...review, language: event.target.value })}
                    value={review.language}
                >
                    {(LANGUAGES.includes(review.language)
                        ? LANGUAGES
                        : [review.language, ...LANGUAGES]
                    ).map((language) => (
                        <option key={language}>{language}</option>
                    ))}
                </select>
            </Row>
        </>
    );
}

function TestRow({ config }: { config: AiConfig }) {
    const [state, setState] = useState<'running' | { message: string; ok: boolean } | null>(null);
    const name = PROVIDER_LABELS[config.provider];

    useEffect(() => setState(null), [config.provider, config.model, config.baseUrl]);

    return (
        <Row
            desc={
                state === 'running'
                    ? `Waiting for ${name}…`
                    : state
                      ? state.message
                      : `Asks ${name} for a one-word answer, the way a review would reach it.`
            }
            title="Test the connection"
        >
            {state && state !== 'running' && (
                <span className={`text-[12px] font-semibold ${state.ok ? 'text-add' : 'text-del'}`}>
                    {state.ok ? 'Works' : 'Failed'}
                </span>
            )}
            <Button
                disabled={state === 'running'}
                onClick={() => {
                    setState('running');
                    api.testAi().then(
                        (result) =>
                            setState({
                                message: `Answered “${result.reply}” in ${(result.durationMs / 1000).toFixed(1)} s.`,
                                ok: true,
                            }),
                        (error: unknown) => setState({ message: errorMessage(error), ok: false }),
                    );
                }}
                size="sm"
            >
                Test
            </Button>
        </Row>
    );
}

function TextSetting({
    desc,
    onCommit,
    placeholder,
    title,
    value,
}: {
    desc: string;
    onCommit(value: string): void;
    placeholder: string;
    title: string;
    value: string;
}) {
    const [draft, setDraft] = useState(value);

    useEffect(() => setDraft(value), [value]);

    const commit = () => draft.trim() !== value && onCommit(draft.trim());

    return (
        <Row desc={desc} title={title}>
            <input
                aria-label={title}
                className={TEXT_FIELD}
                onBlur={commit}
                onChange={(event) => setDraft(event.target.value)}
                onKeyDown={(event) => event.key === 'Enter' && commit()}
                placeholder={placeholder}
                spellCheck={false}
                value={draft}
            />
        </Row>
    );
}

// MARK: Keyboard shortcuts

const SHORTCUTS: { group: string; rows: [string, string][] }[] = [
    {
        group: 'Everywhere',
        rows: [
            ['⌘K', 'Search and run any command'],
            ['⌘O', 'Open a repository'],
            ['⌘,', 'Settings'],
            ['⌘Z / ⇧⌘Z', 'Undo / redo the last git action'],
            ['⌘Q', 'Quit Seam (closing windows keeps it in the menu bar)'],
        ],
    },
    {
        group: 'Views',
        rows: [
            ['⌘1 · ⌘2 · ⌘3 · ⌘4', 'Changes · History · Review · Visual'],
            ['⌘B', 'Show or hide the sidebar'],
            ['⌘L', 'Show or hide the commit panel'],
            ['⇧⌘E', 'Expand the diff (esc to come back)'],
        ],
    },
    {
        group: 'Git',
        rows: [
            ['⌘↵', 'Commit staged changes'],
            ['⇧⌘A', 'Stage all'],
            ['⌥⌘S', 'Stash changes'],
            ['⇧⌘F · ⇧⌘P · ⌘P', 'Fetch · Pull · Push'],
            ['⇧⌘B', 'Switch branch'],
            ['⇧⌘N', 'New branch'],
            ['⇧⌘R', 'Review the branch with AI'],
            ['⇧⌘V', 'Draw a visual recap of the branch'],
        ],
    },
    {
        group: 'In a diff or the commit list',
        rows: [
            ['Click a line number', 'Select a line to stage, unstage or discard'],
            ['⇧-click', 'Select a range of lines, or of commits'],
            ['⌘-click', 'Tick a commit to squash, remove or cherry-pick'],
        ],
    },
];

export function AboutPane() {
    const version = useAppVersion();
    const [git, setGit] = useState('');

    useEffect(() => {
        void api.gitVersion().then(setGit, () => setGit('not found'));
    }, []);

    const rows: [string, string][] = [
        ['Version', version],
        ['Git', git],
        ['Account', 'None. Seam never asks for one'],
        ['Telemetry', 'None'],
        ['AI', 'Only when you start a review, through the agent or provider you chose'],
        ['Built with', 'Tauri 2 · Rust · React 19 · TypeScript'],
        ['Licence', 'AGPL-3.0'],
    ];

    return (
        <div className="flex flex-col items-center gap-3 pt-6 text-center">
            <SeamMark size={64} />
            <div>
                <div className="text-[17px] font-semibold">Seam</div>
                <div className="text-muted">Version {version}</div>
            </div>
            <p className="m-0 max-w-md leading-normal text-soft">
                Everyday git, undoable. Seam runs the git already on your Mac and keeps nothing of
                its own in your repositories beyond the last review, inside .git.
            </p>
            <div className="flex gap-2">
                <Button onClick={() => void openUrl(REPOSITORY_URL)}>Source on GitHub</Button>
                <Button onClick={() => void openUrl(`${REPOSITORY_URL}/releases`)}>
                    Release notes
                </Button>
                <Button onClick={() => void openUrl(`${REPOSITORY_URL}/issues/new`)}>
                    Report an issue
                </Button>
            </div>
            <div className="mt-4 w-full text-left">
                {rows.map(([title, value], index) => (
                    <div
                        className={`flex gap-4 py-2.5 ${index < rows.length - 1 ? 'border-b border-line' : ''}`}
                        key={title}
                    >
                        <span className="w-32 flex-none text-muted">{title}</span>
                        <span className="selectable">{value}</span>
                    </div>
                ))}
            </div>
        </div>
    );
}

// MARK: About

export function ShortcutsPane() {
    return (
        <>
            {SHORTCUTS.map((section, index) => (
                <section key={section.group}>
                    <SectionLabel first={index === 0}>{section.group}</SectionLabel>
                    {section.rows.map(([keys, action], row) => (
                        <div
                            className={`flex items-center gap-4 py-2.5 ${row < section.rows.length - 1 ? 'border-b border-line' : ''}`}
                            key={action}
                        >
                            <span className="flex-1">{action}</span>
                            <kbd className="rounded-md border border-control-line bg-control px-2 py-0.5 font-mono text-[12px] text-soft">
                                {keys}
                            </kbd>
                        </div>
                    ))}
                </section>
            ))}
        </>
    );
}
