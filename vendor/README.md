# Experimental core source build

Source: https://github.com/vercel-labs/json-render
Commit: `3ad381881194e7011ad3ccd6d668033495a06c29`
Package: `packages/core`, upstream version `0.21.0`
License: Apache-2.0, included inside the archive.
Archive SHA-256: `fe6752fc438176b2518dc9af3d035c90fb1e35f0059841c7a3d1e1a211a6e4a6`

This archive contains the actual upstream core APIs, including `experimental_composeSpec` and `experimental_createEvaluator`. Core TypeScript source and `tsup.config.ts` were not modified.

Built in an isolated directory by copying core's `src`, `tsup.config.ts`, `package.json` and the repository `LICENSE`. To avoid installing unrelated monorepo packages, the package's dev dependencies were replaced with `tsup: 8.5.0` and `typescript: 5.9.3`, its dependency was pinned to `zod: 4.3.6`, and its build script was `tsup`. The standalone tsconfig uses `target: ES2022`, `module: ESNext`, `moduleResolution: Bundler`, `strict: true`, `skipLibCheck: true`, and `lib: [ES2022, DOM]`, includes `src` and excludes `src/**/*.test.ts`.

Build commands: `npm install --ignore-scripts`, `npm run build`, `npm pack`. The resulting archive is a local file dependency; the application's package lock records its integrity. No whole monorepo checkout or source build is needed to run this prototype.
