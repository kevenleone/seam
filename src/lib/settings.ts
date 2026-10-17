import { useSyncExternalStore } from 'react';

import type { Mode } from './themes';
import type { PullMode } from './types';

import { api } from './api';
import { accentById, applyTokens, DEFAULT_THEME, themeById } from './themes';

export interface Settings {
    accent: string;
    codeSize: number;
    darkTheme: string;
    gitPath: string;
    ignoreWhitespace: boolean;
    /** File list (Changes) and branches (History). */
    leftPanel: boolean;
    lightTheme: string;
    pullMode: PullMode;
    reduceMotion: boolean;
    /** Commit panel (Changes). */
    rightPanel: boolean;
    splitDiff: boolean;
    statusBar: boolean;
    /** Colours code in diffs for the languages in lib/highlight.ts. */
    syntaxHighlight: boolean;
    /** Colour mode. */
    theme: Theme;
}

export type Theme = 'auto' | 'dark' | 'light';

export const DEFAULT_SETTINGS: Settings = {
    accent: 'lavender',
    codeSize: 12.5,
    darkTheme: DEFAULT_THEME.dark,
    gitPath: '',
    ignoreWhitespace: false,
    leftPanel: true,
    lightTheme: DEFAULT_THEME.light,
    pullMode: 'fastForwardOnly',
    reduceMotion: false,
    rightPanel: true,
    splitDiff: false,
    statusBar: true,
    syntaxHighlight: true,
    theme: 'dark',
};

const KEY = 'seam.settings';
/** The painted token map, for public/theme.js to apply before the app loads. */
const PAINT_KEY = 'seam.paint';
const listeners = new Set<() => void>();

function read(): Settings {
    try {
        return { ...DEFAULT_SETTINGS, ...JSON.parse(localStorage.getItem(KEY) ?? '{}') };
    } catch {
        return DEFAULT_SETTINGS;
    }
}

let current = read();

export function getSettings(): Settings {
    return current;
}

/** Back to the defaults, keeping where the panels were and which git runs. */
export function restoreDefaults(): void {
    updateSettings({
        ...DEFAULT_SETTINGS,
        gitPath: current.gitPath,
        leftPanel: current.leftPanel,
        rightPanel: current.rightPanel,
    });
}

export function updateSettings(patch: Partial<Settings>): void {
    current = { ...current, ...patch };

    try {
        localStorage.setItem(KEY, JSON.stringify(current));
    } catch {
        // Settings still apply for this session.
    }

    applyTheme();
    listeners.forEach((listener) => listener());
}

// Other windows share localStorage; keep them in sync.
window.addEventListener('storage', (event) => {
    if (event.key !== KEY) {
        return;
    }

    current = read();
    applyTheme();
    listeners.forEach((listener) => listener());
});

/** Paints the chosen theme and accent onto the root; every window calls it on change. */
export function applyTheme(): void {
    const mode = effectiveMode();
    const theme = themeById(mode === 'dark' ? current.darkTheme : current.lightTheme, mode);
    const root = document.documentElement;
    const tokens = applyTokens(root, theme, accentById(current.accent));

    root.dataset.theme = mode;
    root.dataset.motion = current.reduceMotion ? 'reduced' : 'full';

    try {
        localStorage.setItem(PAINT_KEY, JSON.stringify({ mode, tokens }));
    } catch {
        // theme.js falls back to the stylesheet's Seam colours.
    }
}

export function effectiveMode(theme: Theme = current.theme): Mode {
    if (theme !== 'auto') {
        return theme;
    }

    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

export function useSettings(): Settings {
    return useSyncExternalStore(
        (listener) => {
            listeners.add(listener);

            return () => listeners.delete(listener);
        },
        () => current,
        () => current,
    );
}

window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', applyTheme);

const RECENTS = 'seam.recents';
const RECENTS_IMPORTED = 'seam.recentsImported';

/**
 * The host keeps the list of recent repositories (the tray needs it with no window open).
 * This is a copy in localStorage, so the palette and welcome window can read it synchronously.
 */
export function recentRepositories(): string[] {
    try {
        return JSON.parse(localStorage.getItem(RECENTS) ?? '[]');
    } catch {
        return [];
    }
}

export async function removeRecent(path: string): Promise<string[]> {
    await api.removeRecentRepository(path);

    return syncRecents();
}

export async function syncRecents(): Promise<string[]> {
    try {
        // Builds before the tray kept recents only here; hand them to the host once.
        if (!localStorage.getItem(RECENTS_IMPORTED)) {
            await api.importRecentRepositories(recentRepositories());
            localStorage.setItem(RECENTS_IMPORTED, '1');
        }
    } catch {
        // Not imported; nothing else depends on it.
    }

    const paths = await api.recentRepositories();

    cacheRecents(paths);

    return paths;
}

function cacheRecents(paths: string[]): void {
    try {
        localStorage.setItem(RECENTS, JSON.stringify(paths));
    } catch {
        // The copy is a convenience; the host still has the list.
    }
}
