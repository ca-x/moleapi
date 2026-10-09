# Embedded HTTP snippet engine

Pinned @scalar/snippetz0.10.5 (MIT), Postman code-generators2.1.1 (Apache-2.0), Postman Collection SDK5.3.1, core-js3.49.0 (MIT) and locked mature browser polyfills. Upstream: https://github.com/scalar/scalar/tree/main/packages/snippetz and https://github.com/zloirock/core-js. This is a generated bundle of mature emitters/polyfills, not MoleAPI language templates. Full dependency licenses accompany this file, plus esbuild's retained legal notices.

Postman upstream: https://github.com/postmanlabs/postman-code-generators. The minimal reproducible patch in tools/snippets/patches uses postman-url-encoder3.0.8 to avoid double encoding Node Native query values; formatting/body/header emission remains upstream. Browser process/path/Buffer shims provide pure values/strings without host filesystem/network/process APIs.

Reproduce: `npm ci --prefix tools/snippets --ignore-scripts` then `npm --prefix tools/snippets run build`. The lockfile retains integrity/source records, manifest records exact generator/polyfill versions and SHA256. Runtime uses embedded JS in isolated Rust QuickJS, without Node/filesystem/network/process globals. Library updates require actual per-target revalidation.

Merged23 language families/55 library variants cover the current Apifox/Postman interface-request lists. Upstream plugin availability and generation smoke tests do not imply per-language compile/run verification or typed SDK/server generation. In particular this catalog has no separate TypeScript/C++ SDK emitters. MoleAPI reports these gaps explicitly.
