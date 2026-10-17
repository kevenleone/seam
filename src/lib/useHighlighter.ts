import { useEffect, useState } from 'react';

export type Highlighter = typeof import('./highlight');

let loaded: Highlighter | null = null;
let loading: null | Promise<Highlighter> = null;

export function loadHighlighter(): Promise<Highlighter> {
    loading ??= import('./highlight');

    return loading.then((module) => (loaded = module));
}

/** The highlighter once its chunk has loaded; null while loading or when highlighting is off. */
export function useHighlighter(enabled: boolean): Highlighter | null {
    const [highlighter, setHighlighter] = useState(loaded);

    useEffect(() => {
        if (!enabled || loaded) {
            return;
        }

        loading ??= import('./highlight');
        void loading.then((module) => {
            loaded = module;
            setHighlighter(module);
        });
    }, [enabled]);

    return enabled ? (highlighter ?? loaded) : null;
}
