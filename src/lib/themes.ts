// The theme registry. A theme is a whole token map: `paintTheme` writes it onto the root
// element, so every `bg-panel` / `text-muted` utility follows without a `dark:` variant.
// Most themes are seeds (the dozen colours a palette publishes) expanded by `buildTokens`.

export type Mode = 'dark' | 'light';

export interface ThemeDefinition {
    id: string;
    mode: Mode;
    name: string;
    tokens: Record<string, string>;
}

interface ThemeSeed {
    /** Additions: a blue from the palette. */
    add: string;
    app: string;
    canvas: string;
    control: string;
    /** Deletions: an orange from the palette, never red against green. */
    del: string;
    dim: string;
    fg: string;
    line: string;
    mode: Mode;
    panel: string;
    soft: string;
}

const channels = (hex: string): [number, number, number] => {
    const match = /^#?([0-9a-f]{6})$/i.exec(hex);

    if (!match) {
        return [0, 0, 0];
    }

    const value = parseInt(match[1], 16);

    return [(value >> 16) & 255, (value >> 8) & 255, value & 255];
};

const toHex = (value: number): string => Math.round(value).toString(16).padStart(2, '0');

/** Linear blend of two `#rrggbb` colours; `amount` is how much of `first` survives. */
export function mix(first: string, second: string, amount: number): string {
    const [r1, g1, b1] = channels(first);
    const [r2, g2, b2] = channels(second);
    const keep = Math.max(0, Math.min(1, amount));

    return `#${toHex(r1 * keep + r2 * (1 - keep))}${toHex(g1 * keep + g2 * (1 - keep))}${toHex(b1 * keep + b2 * (1 - keep))}`;
}

const SHADOW: Record<Mode, string> = { dark: 'rgba(0, 0, 0, 0.5)', light: 'rgba(0, 0, 0, 0.18)' };

function buildTokens(seed: ThemeSeed): Record<string, string> {
    const dark = seed.mode === 'dark';

    return {
        '--add': seed.add,
        '--add-bg': mix(seed.add, seed.app, dark ? 0.13 : 0.1),
        '--add-gutter': mix(seed.add, seed.app, dark ? 0.22 : 0.18),
        '--add-line': mix(seed.add, seed.app, 0.32),
        '--add-num': mix(seed.add, seed.fg, dark ? 0.55 : 0.75),
        '--app': seed.app,
        '--canvas': seed.canvas,
        '--control': seed.control,
        '--control-line': mix(seed.line, seed.dim, 0.88),
        '--del': seed.del,
        '--del-bg': mix(seed.del, seed.app, dark ? 0.13 : 0.1),
        '--del-gutter': mix(seed.del, seed.app, dark ? 0.22 : 0.18),
        '--del-line': mix(seed.del, seed.app, 0.32),
        '--del-num': mix(seed.del, seed.fg, dark ? 0.55 : 0.75),
        '--dim': seed.dim,
        '--fg': seed.fg,
        '--fg-body': mix(seed.fg, seed.soft, 0.55),
        '--fg-soft': seed.soft,
        '--hover': mix(seed.control, seed.fg, 0.97),
        '--hunk': mix(seed.panel, seed.control, 0.5),
        '--line': seed.line,
        '--muted': mix(seed.soft, seed.dim, 0.6),
        '--panel': seed.panel,
        '--selected': mix(seed.control, seed.fg, dark ? 0.93 : 0.95),
        '--shadow': SHADOW[seed.mode],
        '--tab-active': dark ? mix(seed.control, seed.fg, 0.86) : seed.app,
        '--warn-bg': mix(seed.del, seed.app, dark ? 0.09 : 0.08),
        '--warn-fg': mix(seed.del, seed.fg, dark ? 0.6 : 0.8),
    };
}

/** Seam's own themes, hand-tuned to the design canvas rather than derived. */
const SEAM_DARK: Record<string, string> = {
    '--add': '#6cb6ff',
    '--add-bg': '#13243a',
    '--add-gutter': '#183154',
    '--add-line': '#1f4675',
    '--add-num': '#9cc3ee',
    '--app': '#121317',
    '--canvas': '#0b0c0f',
    '--control': '#1d1f26',
    '--control-line': '#2a2c34',
    '--del': '#f0a060',
    '--del-bg': '#2d1b10',
    '--del-gutter': '#3e2515',
    '--del-line': '#4a2e1c',
    '--del-num': '#e8b48c',
    '--dim': '#7a7c86',
    '--fg': '#e7e5e0',
    '--fg-body': '#d9d7d1',
    '--fg-soft': '#b3b4bc',
    '--hover': '#1d1f26',
    '--hunk': '#1a1b21',
    '--line': '#24262d',
    '--muted': '#9c9da6',
    '--panel': '#16171c',
    '--selected': '#23252e',
    '--shadow': SHADOW.dark,
    '--tab-active': '#33353f',
    '--warn-bg': '#24180f',
    '--warn-fg': '#f5b888',
};

const SEAM_LIGHT: Record<string, string> = {
    '--add': '#0b5cad',
    '--add-bg': '#e4effb',
    '--add-gutter': '#cfe2f7',
    '--add-line': '#a9c8ec',
    '--add-num': '#2f5f95',
    '--app': '#fbfaf7',
    '--canvas': '#e9e8e4',
    '--control': '#ffffff',
    '--control-line': '#d6d4cd',
    '--del': '#a3500f',
    '--del-bg': '#fcede1',
    '--del-gutter': '#f6d9c2',
    '--del-line': '#ecc4a4',
    '--del-num': '#8c4a1a',
    '--dim': '#8a8b93',
    '--fg': '#1b1b1f',
    '--fg-body': '#2a2a30',
    '--fg-soft': '#4a4b52',
    '--hover': '#ecebe6',
    '--hunk': '#f0efea',
    '--line': '#e2e0da',
    '--muted': '#6b6c74',
    '--panel': '#f3f2ee',
    '--selected': '#e6e4de',
    '--shadow': SHADOW.light,
    '--tab-active': '#ffffff',
    '--warn-bg': '#fff3e6',
    '--warn-fg': '#8c4a1a',
};

// Palettes as their authors publish them (via Lore's registry), plus each one's own
// blue and orange for the diff.
const SEEDS: { id: string; name: string; seed: ThemeSeed }[] = [
    {
        id: 'midnight',
        name: 'Midnight',
        seed: {
            add: '#8f9bf0',
            app: '#101014',
            canvas: '#050507',
            control: '#17171d',
            del: '#d3a06a',
            dim: '#6a6a76',
            fg: '#ececf1',
            line: '#23232b',
            mode: 'dark',
            panel: '#0a0a0d',
            soft: '#9a9aa6',
        },
    },
    {
        id: 'obsidian',
        name: 'Obsidian',
        seed: {
            add: '#7f95d8',
            app: '#000000',
            canvas: '#000000',
            control: '#131317',
            del: '#c69f6a',
            dim: '#6e6e76',
            fg: '#f2f2f4',
            line: '#202024',
            mode: 'dark',
            panel: '#060607',
            soft: '#9c9ca4',
        },
    },
    {
        id: 'dracula',
        name: 'Dracula',
        seed: {
            add: '#8be9fd',
            app: '#282a36',
            canvas: '#1a1b23',
            control: '#343746',
            del: '#ffb86c',
            dim: '#6272a4',
            fg: '#f8f8f2',
            line: '#44475a',
            mode: 'dark',
            panel: '#21222c',
            soft: '#b6b8c8',
        },
    },
    {
        id: 'catppuccin-mocha',
        name: 'Catppuccin Mocha',
        seed: {
            add: '#89b4fa',
            app: '#1e1e2e',
            canvas: '#11111b',
            control: '#313244',
            del: '#fab387',
            dim: '#7f849c',
            fg: '#cdd6f4',
            line: '#313244',
            mode: 'dark',
            panel: '#181825',
            soft: '#a6adc8',
        },
    },
    {
        id: 'catppuccin-frappe',
        name: 'Catppuccin Frappé',
        seed: {
            add: '#8caaee',
            app: '#303446',
            canvas: '#232634',
            control: '#414559',
            del: '#ef9f76',
            dim: '#838ba7',
            fg: '#c6d0f5',
            line: '#414559',
            mode: 'dark',
            panel: '#292c3c',
            soft: '#a5adce',
        },
    },
    {
        id: 'rose-pine',
        name: 'Rosé Pine',
        seed: {
            add: '#9ccfd8',
            app: '#1f1d2e',
            canvas: '#16141f',
            control: '#26233a',
            del: '#f6c177',
            dim: '#6e6a86',
            fg: '#e0def4',
            line: '#26233a',
            mode: 'dark',
            panel: '#191724',
            soft: '#908caa',
        },
    },
    {
        id: 'nord',
        name: 'Nord',
        seed: {
            add: '#88c0d0',
            app: '#2e3440',
            canvas: '#242933',
            control: '#3b4252',
            del: '#d08770',
            dim: '#7b8494',
            fg: '#eceff4',
            line: '#3b4252',
            mode: 'dark',
            panel: '#292e39',
            soft: '#d8dee9',
        },
    },
    {
        id: 'one-dark-pro',
        name: 'One Dark Pro',
        seed: {
            add: '#61afef',
            app: '#282c34',
            canvas: '#1b1e24',
            control: '#2c313a',
            del: '#d19a66',
            dim: '#5c6370',
            fg: '#dcdfe4',
            line: '#3e4451',
            mode: 'dark',
            panel: '#21252b',
            soft: '#abb2bf',
        },
    },
    {
        id: 'tokyo-night',
        name: 'Tokyo Night',
        seed: {
            add: '#7aa2f7',
            app: '#1a1b26',
            canvas: '#13131a',
            control: '#24283b',
            del: '#ff9e64',
            dim: '#565f89',
            fg: '#c0caf5',
            line: '#292e42',
            mode: 'dark',
            panel: '#16161e',
            soft: '#a9b1d6',
        },
    },
    {
        id: 'solarized-dark',
        name: 'Solarized Dark',
        seed: {
            add: '#268bd2',
            app: '#002b36',
            canvas: '#001f27',
            control: '#073642',
            del: '#cb4b16',
            dim: '#657b83',
            fg: '#eee8d5',
            line: '#0f4451',
            mode: 'dark',
            panel: '#00252e',
            soft: '#93a1a1',
        },
    },
    {
        id: 'gruvbox-dark',
        name: 'Gruvbox Dark',
        seed: {
            add: '#83a598',
            app: '#282828',
            canvas: '#191919',
            control: '#3c3836',
            del: '#fe8019',
            dim: '#928374',
            fg: '#ebdbb2',
            line: '#3c3836',
            mode: 'dark',
            panel: '#1d2021',
            soft: '#bdae93',
        },
    },
    {
        id: 'catppuccin-latte',
        name: 'Catppuccin Latte',
        seed: {
            add: '#1e66f5',
            app: '#eff1f5',
            canvas: '#dce0e8',
            control: '#dce0e8',
            del: '#fe640b',
            dim: '#9ca0b0',
            fg: '#4c4f69',
            line: '#ccd0da',
            mode: 'light',
            panel: '#e6e9ef',
            soft: '#6c6f85',
        },
    },
    {
        id: 'rose-pine-dawn',
        name: 'Rosé Pine Dawn',
        seed: {
            add: '#286983',
            app: '#fffaf3',
            canvas: '#f2e9e1',
            control: '#f2e9e1',
            del: '#d7827e',
            dim: '#9893a5',
            fg: '#575279',
            line: '#dfdad9',
            mode: 'light',
            panel: '#faf4ed',
            soft: '#797593',
        },
    },
    {
        id: 'solarized-light',
        name: 'Solarized Light',
        seed: {
            add: '#268bd2',
            app: '#fdf6e3',
            canvas: '#eee8d5',
            control: '#eee8d5',
            del: '#cb4b16',
            dim: '#93a1a1',
            fg: '#073642',
            line: '#e0d9c3',
            mode: 'light',
            panel: '#f5eeda',
            soft: '#586e75',
        },
    },
    {
        id: 'github-light',
        name: 'GitHub Light',
        seed: {
            add: '#0969da',
            app: '#ffffff',
            canvas: '#eff2f5',
            control: '#eff2f5',
            del: '#bc4c00',
            dim: '#818b98',
            fg: '#1f2328',
            line: '#d1d9e0',
            mode: 'light',
            panel: '#f6f8fa',
            soft: '#59636e',
        },
    },
    {
        id: 'gruvbox-light',
        name: 'Gruvbox Light',
        seed: {
            add: '#076678',
            app: '#fbf1c7',
            canvas: '#ebdbb2',
            control: '#ebdbb2',
            del: '#af3a03',
            dim: '#928374',
            fg: '#3c3836',
            line: '#d5c4a1',
            mode: 'light',
            panel: '#f2e5bc',
            soft: '#504945',
        },
    },
];

export const THEMES: ThemeDefinition[] = [
    { id: 'seam-dark', mode: 'dark', name: 'Seam Dark', tokens: SEAM_DARK },
    { id: 'seam-light', mode: 'light', name: 'Seam Light', tokens: SEAM_LIGHT },
    ...SEEDS.map(({ id, name, seed }) => ({
        id,
        mode: seed.mode,
        name,
        tokens: buildTokens(seed),
    })),
];

export const DEFAULT_THEME: Record<Mode, string> = { dark: 'seam-dark', light: 'seam-light' };

export interface Accent {
    /** Legible on a dark ground. */
    dark: string;
    id: string;
    /** Legible on a light ground. */
    light: string;
    name: string;
}

/** A stale id, or one of the other mode, falls back to the mode's default. */
export function themeById(id: string, mode: Mode): ThemeDefinition {
    return (
        THEMES.find((theme) => theme.id === id && theme.mode === mode) ??
        (THEMES.find((theme) => theme.id === DEFAULT_THEME[mode]) as ThemeDefinition)
    );
}

export function themesFor(mode: Mode): ThemeDefinition[] {
    return THEMES.filter((theme) => theme.mode === mode);
}

export const ACCENTS: Accent[] = [
    { dark: '#b9a8ff', id: 'lavender', light: '#6b55d6', name: 'Lavender' },
    { dark: '#9b9bf0', id: 'indigo', light: '#5b5bd6', name: 'Indigo' },
    { dark: '#7fc39a', id: 'fern', light: '#3f8f6a', name: 'Fern' },
    { dark: '#6cc3c3', id: 'teal', light: '#237a7a', name: 'Teal' },
    { dark: '#e48b74', id: 'clay', light: '#c4553d', name: 'Clay' },
    { dark: '#d3ac72', id: 'ochre', light: '#96661f', name: 'Ochre' },
    { dark: '#a9b0d4', id: 'slate', light: '#5d6690', name: 'Slate blue' },
    { dark: '#b8bac9', id: 'graphite', light: '#393a4a', name: 'Graphite' },
];

export function accentById(id: string): Accent {
    return ACCENTS.find((accent) => accent.id === id) ?? ACCENTS[0];
}

/** The accent's tokens on a given theme's ground. */
export function accentTokens(accent: Accent, theme: ThemeDefinition): Record<string, string> {
    const ground = theme.tokens['--app'];

    if (theme.mode === 'dark') {
        const color = accent.dark;

        return {
            '--accent': color,
            '--accent-bg': mix(color, ground, 0.08),
            '--accent-line': mix(color, ground, 0.28),
            '--accent-strong': mix(color, '#ffffff', 0.6),
            '--on-accent': mix(color, '#000000', 0.12),
        };
    }

    const color = accent.light;

    return {
        '--accent': color,
        '--accent-bg': mix(color, ground, 0.09),
        '--accent-line': mix(color, ground, 0.3),
        '--accent-strong': mix(color, '#000000', 0.85),
        '--on-accent': '#ffffff',
    };
}

/** Writes a theme (and accent) onto an element; the root for the app, a card for a preview. */
export function applyTokens(
    element: HTMLElement,
    theme: ThemeDefinition,
    accent: Accent,
): Record<string, string> {
    const tokens = { ...theme.tokens, ...accentTokens(accent, theme) };

    for (const [name, value] of Object.entries(tokens)) {
        element.style.setProperty(name, value);
    }

    element.style.colorScheme = theme.mode;

    return tokens;
}
