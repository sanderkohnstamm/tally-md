import js from '@eslint/js';
import globals from 'globals';

export default [
  js.configs.recommended,
  {
    files: ['src-ui/**/*.js'],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: 'module',
      globals: {
        ...globals.browser,
      },
    },
    rules: {
      // Strict equality
      eqeqeq: ['error', 'always', { null: 'ignore' }],

      // No unused variables (error, not warn)
      'no-unused-vars': ['error', { argsIgnorePattern: '^_', varsIgnorePattern: '^_' }],

      // Prevent common bugs
      'no-var': 'error',
      'prefer-const': 'error',
      'no-implicit-coercion': 'error',
      'no-throw-literal': 'error',
      'no-self-compare': 'error',
      'no-template-curly-in-string': 'error',
      'no-unmodified-loop-condition': 'error',
      'no-unreachable-loop': 'error',
      'no-constant-binary-expression': 'error',
      'no-constructor-return': 'error',
      'no-promise-executor-return': 'error',
      'no-new-native-nonconstructor': 'error',

      // Code quality
      'no-else-return': 'error',
      'no-lonely-if': 'error',
      'no-nested-ternary': 'error',
      'no-unneeded-ternary': 'error',
      'prefer-template': 'error',
      'object-shorthand': 'error',
      'prefer-arrow-callback': 'error',
      'no-useless-rename': 'error',
      'no-useless-return': 'error',
      'no-useless-concat': 'error',
      'no-useless-computed-key': 'error',

      // No console in production
      'no-console': 'warn',

      // No eval / implied eval
      'no-eval': 'error',
      'no-implied-eval': 'error',
      'no-new-func': 'error',

      // No shadow
      'no-shadow': 'error',

      // Async
      'no-await-in-loop': 'warn',
      'require-atomic-updates': 'error',
    },
  },
];
