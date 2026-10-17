import { getName, getVersion } from '@tauri-apps/api/app';
import { useEffect, useState } from 'react';

function cachedHook(load: () => Promise<string>): () => string {
    let cached: string | undefined;

    return function useCached(): string {
        const [value, setValue] = useState(cached ?? '');

        useEffect(() => {
            if (cached) {
                return;
            }

            void load().then((loaded) => {
                cached = loaded;
                setValue(loaded);
            });
        }, []);

        return value;
    };
}

export const useAppName = cachedHook(getName);

export const useAppVersion = cachedHook(getVersion);
