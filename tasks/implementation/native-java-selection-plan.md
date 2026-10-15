# Native Java selection and persistence

Parent: remaining-work-plan milestone2 and docs/specs/code-generation.md.
Execute inline in the existing application worktree. Hosted runtime configuration
remains deployment-owned; no HTTP route can select or execute a new program.

Goal: a desktop user can select Java17+ using the OS file dialog, validate it,
persist it privately, and generate immediately and after restart.

## Implementation and acceptance

- [x] Keep ProjectRuntime immutable except a shared Java-path selection behind a
  short standard RwLock. Each generation clones its selected path before awaiting,
  so changing the choice preserves in-flight jobs and cancellation state.
- [x] Add native-only preference helpers to the generation crate: capped/cleared-env
  Java -version probe with5-second timeout, require successful Java17+ version,
  bounded versioned JSON preference, atomic private write before updating selection.
  Invalid files/old Java/probe failures retain both prior choice and preference.
- [x] Add a local embedder constructor accepting the trusted ProjectRuntime clone;
  existing hosted/local/headless constructors preserve current behavior.
- [x] Wire a Tauri command using its Rust system file dialog, serialize picker work,
  restore the trusted preference at startup, and share the runtime with the router.
  Cancellation returns unchanged state. No arbitrary path argument from the webview.
- [x] Add a focused native Java configuration row inside the current generation
  dialog. Apply ui-ux-pro-max and emil-design-eng: existing Radix tokens, descriptive
  action, loading/inline error/status, stable layout, keyboard access, no new motion.
  Refresh catalog on successful selection; fence late replies after close/source/
  account changes. Hosted users retain deployment guidance and see no native action.
- [x] Finish implementation and tests before executing them. Test real selection/
  persistence/restart, rejection/timeout/output bounds, unchanged prior configuration,
  immediate generated artifact using a cloned runtime, native IPC adapter/UI cancel/
  success/error/stale reply, and hosted absence. Test source preservation and i18n.
- [x] Run generation/API/UI/i18n/build/lint/format gates and actual browser checks
  in English/light and Chinese/dark at desktop/mobile widths. Check native compilation
  where toolchains permit; keep actual OS-dialog/platform evidence explicitly scoped.
- [x] Update usage/coverage and integrate the verified slice independently of the
  pending Apple RPC/model batch. Bundled Java and installer/signing remain open.

Review focus: untrusted specs cannot configure executables; stale UI must not enable
the wrong source; failed persistence cannot change active choice; cancellation leaves
preferences untouched; two generations can keep their original selected executable.


## Execution ledger

- Implementation and all test code preceded runtime tests. The chosen runtime path
  is shared through a short RwLock; the desktop serializes explicit selections.
- Passed83 generation regressions (44 external cases ignored),2 model/local API cases,
 1 actual JDK21 concurrency/restart case with invalid inherited JAVA_TOOL_OPTIONS,
 285 frontend cases, production frontend build, native cargo check and scoped strict
  Clippy. First Clippy attempt was discarded when concurrent frontend build changed
  embedded resource inputs; a stable-input rerun passed.
- Browser harness (explicit native-bridge test doubles) checks English/light1280px,
  Chinese/dark375px and812px landscape, reduced motion,44px button, keyboard focus,
  quiet cancellation and inline errors. No horizontal overflow; scoped axe0 violations.
- Real native compile/dialog work uses isolated official signed WebKitGTK packages;
  no host packages or user application data is changed. Native full build and actual GTK picker/generation/restart now pass (details below).
- Independent Apple protobuf/model runs remain queued; neither is claimed complete.
  SQL CI38688180724/database116267801306 passed4 original-DDL cases on16.15/8.4.11.

- Actual production-asset Linux/Xvfb native flow: imported canonical source; selected
  JDK21 in real GTK picker; verified exact JSON preference/mode0600; canceled another
  picker; rejected /usr/bin/false with inline error and unchanged preference; generated
  45 C# files, restarted actual application and generated45 files again. Evidence:
  /tmp/moleapi-native-java-native-*.png and native-evidence.json.14 API cases passed.
- QA relocation ruling: both prefix and libdir in isolated WebKit pkg-config metadata
  must be relocated. Native helpers require FHS mounts; overlay host-library symlinks
  stay under /usr/local/lib because glycin's nested sandbox binds /usr, while /mnt
  targets disappear there and crash GTK icon loading. SDK libraries are visible in
  the QA FHS overlay so env-cleared app workers also resolve native dependencies.
  All mounts are ephemeral/read-only; host SDK and user data remain untouched.
- Frontend final disabled-state feedback was improved for missing Java/unsupported
  native3.1 targets;14 focused and285 full frontend cases pass on the final source.
  Other OS/Java packaging, Apple protobuf/model runtime and full goal remain open.
