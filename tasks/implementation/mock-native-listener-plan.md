# Native public Mock listener implementation

Parent: `tasks/implementation/remaining-work-plan.md` milestone 4, `docs/specs/mock-public-access.md`, and `docs/specs/mock-native-listener.md`.

1. [ ] Add a local-only listener lifecycle to `AppState` without a task/lifetime ownership cycle. Bind only explicit loopback or explicit all-interface address and mount only public Mock routes; status/start/stop have no hosted side effects. Test real socket start/stop, occupied port and owner management isolation.
2. [ ] Add cancellation fencing for in-flight public calls on stop and recheck share/workspace behavior through the same hosted engine. Verify delayed and dynamic calls release budgets, listener drop closes the port and LAN opt-in is explicit.
3. [ ] Expose bilingual native controls and the actual listener origin in the Mock share dialog; keep hosted UI behavior and one-time token privacy. Verify real 390px desktop/browser surrogate and installed desktop packages.
4. [ ] Run relevant suites, strict format/Clippy/build, configured database/Docker/native CI and independent review. Merge only with exact-head evidence; record remaining HTTPS/custom-domain/organization limits.

Decision: a listener task needs an `AppState` clone for the shared Mock runtime. To avoid `Hub -> JoinHandle -> AppState -> Hub`, `AppState` stores a `Weak<ListenerHub>` while the outer local Router owns the strong `Arc<ListenerHub>` through an extension. The public TCP Router receives only the weak-bearing state clone. Dropping the outer Router drops the hub, cancels and aborts its task; explicit stop awaits it with a deadline.

Source checkpoint, 2026-10-18: the separate native socket and UI controls are implemented on top of the merged hosted share branch. Three local real-socket tests passed, covering explicit loopback/LAN start, owner IPC isolation, stop/restart/router drop and cancellation during delay. Full server/front-end 366 suites, TypeScript build, strict all-target Clippy and embedded server build passed locally after the stop/start serialization fix. Actual native graphical QA, installed-platform evidence and independent review remain open.
