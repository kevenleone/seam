import { baseConfig } from './eslint.base.mjs';

export default [
    ...baseConfig,
    {
        rules: {
            '@typescript-eslint/no-unused-vars': [
                'error',
                { argsIgnorePattern: '^_', varsIgnorePattern: '^_' },
            ],
        },
    },
    {
        files: ['public/**/*.js'],
        languageOptions: {
            globals: { document: 'readonly', localStorage: 'readonly', matchMedia: 'readonly' },
        },
    },
    {
        ignores: [
            '**/*.config.js',
            '**/*.config.mjs',
            '**/build/**',
            '**/coverage/**',
            '**/dist/**',
            '**/node_modules/**',
            '**/src-tauri/gen/**',
            '**/src-tauri/target/**',
        ],
    },
];
