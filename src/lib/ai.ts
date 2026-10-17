import { listen } from '@tauri-apps/api/event';
import { useCallback, useEffect, useState } from 'react';

import type { AiConfig, Provider, ProviderStatus, ReviewFocusArea, ReviewStyle } from './types';

import { api } from './api';

export const PROVIDER_LABELS: Record<Provider, string> = {
    anthropic: 'Anthropic',
    claude: 'Claude Code',
    codex: 'Codex',
    google: 'Google Gemini',
    openai: 'OpenAI',
    'openai-compatible': 'OpenAI-compatible endpoint',
    opencode: 'OpenCode',
};

export const REVIEW_FOCUS: { desc: string; id: ReviewFocusArea; title: string }[] = [
    { desc: 'Wrong conditions, edge cases, null handling', id: 'bugs', title: 'Bugs and logic' },
    { desc: 'Untrusted input, secrets, auth checks', id: 'security', title: 'Security' },
    { desc: 'Changed behaviour with no test touching it', id: 'tests', title: 'Missing tests' },
    { desc: 'Public APIs, response shapes, configs', id: 'breaking', title: 'Breaking changes' },
    { desc: 'Extra queries, loops, blocking calls', id: 'performance', title: 'Performance' },
    { desc: 'Naming, dead code, long functions', id: 'style', title: 'Style and readability' },
];

export const REVIEW_LEVELS: Record<ReviewStyle['level'], string> = {
    balanced: 'Balanced',
    blockers: 'Blockers only',
    thorough: 'Thorough',
};

/** The AI configuration the host keeps, and every provider's status, kept fresh across windows. */
export function useAi(): {
    config: AiConfig | null;
    providers: ProviderStatus[];
    refresh(): void;
    save(patch: Partial<AiConfig>): Promise<void>;
} {
    const [config, setConfig] = useState<AiConfig | null>(null);
    const [providers, setProviders] = useState<ProviderStatus[]>([]);

    const refresh = useCallback(() => {
        void api.aiConfig().then(setConfig);
        void api.aiProviders().then(setProviders);
    }, []);

    useEffect(() => {
        refresh();

        const off = listen('ai-config-changed', refresh);

        return () => void off.then((stop) => stop());
    }, [refresh]);

    const save = async (patch: Partial<AiConfig>) => {
        const current = config ?? (await api.aiConfig());
        const next = { ...current, ...patch };

        setConfig(next);
        await api.setAiConfig(next);
    };

    return { config, providers, refresh, save };
}
