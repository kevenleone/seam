import { createContext, useContext } from 'react';

import type { Highlighter } from '../../lib/useHighlighter';

/** `app` is interactive; `print` and `export` lay everything out flat for paper and files. */
export type RenderMode = 'app' | 'export' | 'print';

export interface VisualContext {
    highlighter: Highlighter | null;
    mode: RenderMode;
}

export const Visual = createContext<VisualContext>({ highlighter: null, mode: 'app' });

export function useVisual(): VisualContext {
    return useContext(Visual);
}
