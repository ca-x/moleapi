# Embedded HTTP snippet engine

Pinned @scalar/snippetz0.10.5 (MIT), core-js3.49.0 (MIT), their locked transitive dependencies. Upstream: https://github.com/scalar/scalar/tree/main/packages/snippetz and https://github.com/zloirock/core-js. This is a generated bundle of mature emitters/polyfills, not MoleAPI language templates. Full dependency licenses accompany this file, plus esbuild's retained legal notices.

Reproduce: `npm ci --prefix tools/snippets --ignore-scripts` then `npm --prefix tools/snippets run build`. The lockfile retains integrity/source records, manifest records exact generator/polyfill versions and SHA256. Runtime uses embedded JS in isolated Rust QuickJS, without Node/filesystem/network/process globals. Library updates require actual per-target revalidation.

22 upstream language families with library variants. Upstream plugin availability and generation smoke tests do not imply per-language compile/run verification or typed SDK/server generation. In particular this catalog has no separate TypeScript/C++ SDK emitters. MoleAPI reports these gaps explicitly.
