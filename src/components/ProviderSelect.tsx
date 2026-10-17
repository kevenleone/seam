import type { Provider, ProviderStatus } from '../lib/types';

import { useAi } from '../lib/ai';

/** The model to run with, saved to Settings › AI so every AI feature uses the same one. */
export function ProviderSelect({ disabled }: { disabled?: boolean }) {
    const { config, providers, save } = useAi();

    if (!config) {
        return null;
    }

    const usable = providers.filter(
        (provider) => provider.available || provider.id === config.provider,
    );

    return (
        <label className="flex items-center gap-1.5 text-[12px] text-muted">
            With
            <select
                className="h-[26px] w-[170px] rounded-md border border-control-line bg-control px-1.5 text-fg"
                disabled={disabled}
                onChange={(event) =>
                    void save({ model: '', provider: event.target.value as Provider })
                }
                value={config.provider}
            >
                {usable.map((provider) => (
                    <option key={provider.id} value={provider.id}>
                        {providerOption(provider)}
                    </option>
                ))}
            </select>
        </label>
    );
}

function providerOption(provider: ProviderStatus): string {
    if (provider.kind === 'api' && provider.id !== 'openai-compatible' && !provider.hasKey) {
        return `${provider.label} (add a key in Settings)`;
    }

    return provider.available ? provider.label : `${provider.label} (not installed)`;
}
