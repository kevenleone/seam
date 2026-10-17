import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { useEffect } from 'react';

import { SettingsModal } from '../components/settings/SettingsModal';
import { type SettingsPane, useSettingsModal } from '../lib/settingsModal';

/** Settings in their own window, opened from the welcome window or the menu bar. */
export function SettingsWindow() {
    useEffect(() => {
        const pane = new URLSearchParams(location.search).get('pane') as null | SettingsPane;

        useSettingsModal.getState().show(pane ?? 'general');
        void getCurrentWebviewWindow().setTitle('Seam Settings');

        const off = getCurrentWebviewWindow().listen<SettingsPane>('show-pane', (event) =>
            useSettingsModal.getState().show(event.payload),
        );

        return () => void off.then((stop) => stop());
    }, []);

    return <SettingsModal standalone />;
}
