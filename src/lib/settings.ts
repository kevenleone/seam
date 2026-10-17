import { useSyncExternalStore } from 'react';

import type { PullMode } from './types';

export interface Settings {
    codeSize: number;
    gitPath: string;
    ignoreWhitespace: boolean;
    /** File list (Changes) and branches (History). */
    leftPanel: boolean;
    pullMode: PullMode;
    /** Commit panel (Changes). */
    rightPanel: boolean;
    splitDiff: boolean;
    theme: Theme;
}

export type Theme = 'auto' | 'dark' | 'light';

const defaults: Settings = {
    codeSize: 12.5,
    gitPath: '',
    ignoreWhitespace: false,
    leftPanel: true,
    pullMode: 'fastForwardOnly',
    rightPanel: true,
    splitDiff: false,
    theme: 'dark',
};
const KEY = 'seam.settings';
const listeners = new Set<() => void>();

function read(): Settings {
    try {
        return { ...defaults, ...JSON.parse(localStorage.getItem(KEY) ?? '{}') };
    } catch {
        return defaults;
    }
}

let current = read();

export function getSettings(): Settings {
    return current;
}

export function updateSettings(patch: Partial<Settings>) {
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

export function applyTheme() {
    const { theme } = current;
    const dark =
        theme === 'dark' ||
        (theme === 'auto' && window.matchMedia('(prefers-color-scheme: dark)').matches);

    document.documentElement.dataset.theme = dark ? 'dark' : 'light';
}

export function useSettings(): Settings {
    return useSyncExternalStore(
        (listener) => {
            listeners.add(listener);

            return () => listeners.delete(listener);
        },
        () => current,
    );
}

window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', applyTheme);

const RECENTS = 'seam.recents';

export function addRecent(path: string) {
    const list = [path, ...recentRepositories().filter((recent) => recent !== path)].slice(0, 12);

    try {
        localStorage.setItem(RECENTS, JSON.stringify(list));
    } catch {
        // Not remembered; nothing else depends on it.
    }
}

export function recentRepositories(): string[] {
    try {
        return JSON.parse(localStorage.getItem(RECENTS) ?? '[]');
    } catch {
        return [];
    }
}

export function removeRecent(path: string) {
    try {
        localStorage.setItem(
            RECENTS,
            JSON.stringify(recentRepositories().filter((recent) => recent !== path)),
        );
    } catch {
        // Ignore.
    }
}
