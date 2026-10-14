# Generated Browser SDK Runtime Plan

Parent: `docs/specs/generated-project-runtime-fixtures.md`.

1. Add an explicit real-browser consumer harness for default JavaScript and TypeScript-fetch SDKs, core/binary/multipart/union profiles and3.0/3.1. Generate fresh projects through the shared runtime, bundle browser entrypoints with standard esbuild and serve same-origin trusted fixtures.
2. Author generated-client/model calls and browser-native File/Blob/FormData checks using the existing exact wire contracts. Run through an isolated agent-browser session, record actual user agent/profile/engine/source evidence and close owned browser/server resources. Finish source/tests before execution.
3. Execute16 client/profile/dialect combinations, diagnose concrete packaging/transport/model failures without substituting Node or mocks, fix mature integration if needed, then run generation/lint/format gates. Document tested browser and remaining cross-origin/platform/advanced limits; commit/merge/push verified work.

Verification: all16 real-browser client/profile/dialect combinations passed (76 API calls) with fresh generated SDKs, standard browser bundling and native File/Blob payloads; no new engine defect was found. The final harness records actual Chromium155 user agent, agent-browser0.38.2/esbuild0.28.2 and source/bundle hashes. Generation65 passed/12 configured external-tool cases ignored; strict scoped Clippy and format/diff gates passed. Other browsers/WebViews/CORS/cookies/advanced streams/shapes and remaining ecosystem/platform/install work remain open.
