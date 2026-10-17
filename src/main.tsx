import { getCurrentWindow } from '@tauri-apps/api/window';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { api } from './lib/api';
import { applyTheme, getSettings } from './lib/settings';
import { RepoWindow } from './views/RepoWindow';
import { Welcome } from './views/Welcome';
import './styles.css';

applyTheme();
void api.setGitPath(getSettings().gitPath || null);

// Each window loads index.html with ?view=welcome or ?repo=<path>.
const params = new URLSearchParams(location.search);
const repo = params.get('repo');

// No browser context menu (Reload, Inspect…) except in text fields.
document.addEventListener('contextmenu', (event) => {
    const t = event.target as HTMLElement;

    if (!(t.tagName === 'INPUT' || t.tagName === 'TEXTAREA')) {
        event.preventDefault();
    }
});

createRoot(document.getElementById('root')!).render(
    <StrictMode>{repo ? <RepoWindow root={repo} /> : <Welcome />}</StrictMode>,
);

// The window was created hidden. Show it once React's first frame is on screen
// (two frames: one to commit, one to paint) so it never flashes an empty webview.
requestAnimationFrame(() =>
    requestAnimationFrame(() => {
        const win = getCurrentWindow();

        void win.show().then(() => win.setFocus());
    }),
);
