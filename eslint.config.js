import tseslint from "typescript-eslint";

export default tseslint.config(
  { ignores: ["dist/**", "out/**", "node_modules/**", "artifacts/**", "host/**", "py_modules/**", "src/dev/**"] },
  ...tseslint.configs.strict,
  {
    files: ["src/**/*.{ts,tsx}", "mockup/**/*.{ts,tsx}", "vite.config.ts"],
    languageOptions: { parserOptions: { ecmaFeatures: { jsx: true } } },
    rules: {
      "@typescript-eslint/consistent-type-imports": "error",
      "@typescript-eslint/no-explicit-any": "error"
    }
  }
);
