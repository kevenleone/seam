import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// Tauri expects a fixed dev port and doesn't need Vite to clear the terminal.
export default defineConfig({
    build: { outDir: 'dist', target: 'safari18' },
    clearScreen: false,
    plugins: [react(), tailwindcss()],
    server: { port: 1420, strictPort: true, watch: { ignored: ['**/src-tauri/**'] } },
});
