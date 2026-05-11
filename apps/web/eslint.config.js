import js from '@eslint/js';
import svelte from 'eslint-plugin-svelte';
import tseslint from 'typescript-eslint';
import svelteParser from 'svelte-eslint-parser';

export default tseslint.config(
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...svelte.configs['flat/recommended'],
  {
    files: ['**/*.svelte'],
    languageOptions: {
      parser: svelteParser,
      parserOptions: {
        parser: tseslint.parser
      }
    }
  },
  {
    rules: {
      'no-undef': 'off', // TypeScript handles undefined-variable checks more accurately
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_' }
      ],
      '@typescript-eslint/consistent-type-imports': 'error',
      'svelte/no-unused-svelte-ignore': 'error',
      'svelte/no-navigation-without-resolve': 'off'
    }
  },
  {
    ignores: ['.svelte-kit/**', 'build/**', 'node_modules/**']
  }
);
