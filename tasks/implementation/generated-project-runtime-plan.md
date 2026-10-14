# Generated Project Runtime Plan

Spec: `docs/specs/generated-project-runtime-fixtures.md`.

1. Add a trusted fixture-project generator using ProjectRuntime, synthetic3.0/3.1 schemas and explicit application/Java executables.
2. Write isolated dependency/build/call drivers for JavaScript/Python/Go/TypeScript clients plus FastAPI skeleton start/respond. Check wire auth/query/header/path/JSON and generated enum/nullable/error behavior.
3. Run the finished harness with available tools, resolve actual failures, and rerun only affected cases before the final batch gate. Record exact versions, target/dialect evidence and remaining limitations. Commit/merge/push completed verified work.

Verification: all10 target/dialect combinations passed (8 real client-call cases plus2 FastAPI startup/route cases); Go ran with race checking. Generation regression55 passed/6 configured external-tool cases ignored; scoped strict Clippy and format/diff gates passed. Actual generated method signatures differ between3.0/3.1; JavaScript drivers bind from generated signatures and Python uses keyword arguments. Other ecosystems/frameworks, advanced keyword/file/union and packaging remain open.
