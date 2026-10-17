import { getVersion } from '@tauri-apps/api/app';
import { useEffect, useState } from 'react';

let cached: string | undefined;

export function useAppVersion(): string {
    const [version, setVersion] = useState(cached ?? '');

    useEffect(() => {
        if (cached) {
            return;
        }

        void getVersion().then((value) => {
            cached = value;
            setVersion(value);
        });
    }, []);

    return version;
}
