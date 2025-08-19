// apps/frontend/eslint.config.mjs
import { FlatCompat } from '@eslint/eslintrc';
import js from '@eslint/js';
import tsPlugin from '@typescript-eslint/eslint-plugin';
import nextPlugin from '@next/eslint-plugin-next';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const compat = new FlatCompat({
  // lets FlatCompat resolve legacy shareable configs from this folder
  baseDirectory: __dirname,
});

const config = [
  // 1) Ignore build artifacts
  {
    ignores: ['node_modules/**', '.next/**', 'out/**', 'dist/**', 'coverage/**', 'pnpm-lock.yaml'],
  },

  // 2) Base JS recommended (flat-native)
  js.configs.recommended,

  // 3) Bring in your legacy extends (Next core-web-vitals, TS recommended, Prettier)
  ...compat.extends('next/core-web-vitals', 'plugin:@typescript-eslint/recommended', 'prettier'),

  // 4) Your custom rules, env, settings
  {
    files: ['**/*.{ts,tsx,js,jsx}'],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: 'module',
    },
    // ensure plugin is available for rules below in flat config
    plugins: {
      '@typescript-eslint': tsPlugin,
      '@next/next': nextPlugin,
    },
    settings: {
      // Helps Next plugin find your app in a monorepo
      next: { rootDir: ['apps/frontend/'] },
      react: { version: 'detect' },
    },
    rules: {
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_' },
      ],
      '@typescript-eslint/consistent-type-imports': 'warn',
      'no-console': ['warn', { allow: ['warn', 'error'] }],
      '@next/next/no-html-link-for-pages': ['error', ['app']],
    },
  },
];

export default config;
