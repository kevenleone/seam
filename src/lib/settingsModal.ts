import { create } from 'zustand';

export type SettingsPane = 'about' | 'ai' | 'appearance' | 'general' | 'shortcuts';

interface SettingsModalState {
    close(): void;
    open: boolean;
    pane: SettingsPane;
    show(pane?: SettingsPane): void;
}

export const useSettingsModal = create<SettingsModalState>()((set) => ({
    close: () => set({ open: false }),
    open: false,
    pane: 'general',
    show: (pane) => set((state) => ({ open: true, pane: pane ?? state.pane })),
}));
