# Frontend foundation implementation report

Completed in `web/` on the application worktree. No commits or backend changes made by this agent.

## Module boundaries

`App.tsx` composes the auth/boot states, workbench provider, layout and dialogs. Feature implementations live under `src/features/auth`, `workspaces`, `requests`, `environments`, `history`, `testing`, `specifications`, `sync`, `interchange`, `settings`, and `workbench`. These contain active hooks, pages, editor panels or dialog content; there are no decorative placeholder modules. Shared API transport, snake_case TypeScript contracts, request factories/cURL templates and Radix/CodeMirror controls are under `src/shared`.

The workbench controller composes feature hooks and owns navigation, cross-feature guards and keyboard commands. Request execution, workspace saving/conflicts, runner execution, history querying, native synchronization, interchange and authentication each have their own controller. Response, assertions, examples, search, settings, connection fields and import/export fields are separate components.

React, Radix UI Themes, TanStack Query, CodeMirror, Lucide and Sonner remain the component/framework foundation. All dialogs, selects, menus, tabs and tooltips use Radix. Test dependencies are Testing Library and jsdom; package lock was updated.

## Usable behavior and fixes

- Existing workspace CRUD, collections/requests, params/headers/body/auth/settings, environment secret markers, request execution, responses/tests/examples, docs, retained original specifications, history, sequential collection testing, import/export and native sync controls preserved against the backend contract.
- The initially displayed runner collection executes on the first click. Runner reports remain scoped to their collection, workspace and account.
- Delayed execute responses/errors never appear under a different selected request/workspace/account. Another dispatch is disabled while a request is in flight.
- Saves serialize concurrent calls, acknowledge the exact sent snapshot and preserve edits typed during the network wait. Unsaved changes remain dirty against the saved snapshot.
- CAS failures retain the full editable workspace and expose the fetched current revision. Users can retain their edits for an explicit full-workspace retry, choose the server version, or download their unsaved workspace backup. The backup action explicitly states that it includes secrets.
- Save-before-switch guards remain open when newer edits were made during saving. New/import operations guard an existing dirty workspace. Failed settings saves keep the dialog open.
- Synchronization/export abort when saving leaves newer unsaved edits. Edits made during sync are retained and the returned database version is offered for resolution.
- Failed save/network/async actions preserve draft data and report errors. Oversized hosted file imports report the 5 MiB limit instead of silently ignoring the file.
- Hosted workspace/history caches and editable drafts are account scoped. An expired-session draft remains available in memory when returning to that account; another login cannot display it. A delayed 401 from an older token does not end a newer session.
- cURL templates put query parameters into the URL before fragments, preserve option placement, safely quote shell literals, redact marked/known credential headers/query fields and URL userinfo, and use auth placeholders.
- History explains that endpoint-returned response bodies may contain sensitive information and can be cleared.

## Interaction and visual rules applied

Read `docs/UI-RESEARCH.md`, `design-system/moleapi/MASTER.md`, and the requested ui-ux-pro-max, emil-design-eng, and animate skills. Ran the ui-ux-pro-max keyboard-focus UX lookup and used its verified focus visibility guidance.

| Before | After | Why |
| --- | --- | --- |
| Monolithic app state and view tree | Composition plus feature controllers/pages | Functional modules can evolve independently |
| Response shared across selection changes | Results keyed to account/workspace/request | Prevents reading another request's result |
| Runner first click only selected a collection | Effective default collection executes immediately | Displayed selection matches the action |
| Save conflict only showed a toast | Radix recovery dialog and local-edit backup | Preserves and exposes recoverable edits |
| Dialog transition rules missed Radix base classes | 200 ms entry / 150 ms exit on actual Radix classes | Consistent occasional pointer state feedback |
| Keyboard triggers inherited motion | Keyboard/command actions are instant | Frequent developer actions stay immediate |
| Crowded narrow-screen header | Compact header, mobile directory toggle with expanded/control semantics | Keeps the workbench usable at small widths |

Motion purpose is state indication for occasional pointer dialogs; CSS opacity/scale uses the existing strong `--ease-out` token. Pointer press feedback is 160 ms. Command search, keyboard actions, frequent selects/menus and tab/list selection do not animate. Sonner transitions are capped at 200 ms. Reduced motion removes dialog/button movement while preserving toast positioning. Fine-pointer hover gating and coarse-pointer 44 px targets remain. The transparent Mole logo, bundled IBM Plex Sans/JetBrains Mono, method colors, clear headings, light/dark themes and static response reading area remain.

## Verification

Executed successfully from `web/` on 2026-10-02:

- `npm run typecheck`
- `npm run test`: 17 tests in four files
- `npm run build`: production output in `web/dist`; no chunk-size warning after editor/Radix vendor splitting

Tests exercise cURL quote round trips/redaction/query fragments/options; saved-snapshot versus in-flight typing; complete edit retention after CAS/network failures; duplicate-save serialization; explicit conflict revision retry; first-click collection execution; delayed response selection safety; changes during synchronization; old conflicts after workspace switching; account-specific draft recovery; and stale/current-token 401 behavior.

Production JavaScript chunks are approximately 386 kB app, 195 kB Radix and 425 kB editor before gzip. Existing Radix theme CSS is approximately 694 kB before gzip. No new motion or modal framework was added.

## Limits and remaining integration checks

This is the usable HTTP workbench foundation, not completion of the 395-feature matrix. Frontier protocols, script runtimes, scenarios, datasets, advanced Mock, document publication, collaboration, local-only Vault/overrides and other matrix capabilities are not represented as implemented. Secret markers hide values and inform export behavior; values remain in the active database. Specification source/dialect and optional request metadata use the current contract unchanged.

Hosted browser verification was initially deferred to root's integration pass; the follow-up below records the completed layout/request checks. Native IPC, native file dialogs and live synchronization were not run from a Tauri desktop here. Draft recovery is in-memory plus unload guarding/explicit backup, not crash-persistent recovery. Conflict retry submits the complete local workspace rather than performing an automatic field merge. The runner remains sequential and request execution has no cancellation control.


## Follow-up: short-height split layout and browser QA (2026-10-03)

Installed and checked stable `react-resizable-panels` 4.14.1's current `Group`, `Panel`, and `Separator` API. The method/URL/actions stay outside a vertical split; request options receive 35% and response 65% by default. Each panel scrolls independently. The visible separator provides the library's pointer and keyboard resizing, ARIA values and focus handling. No drag logic was written by the application. Empty Params no longer reserves a 256 px minimum. Mobile uses a method/URL row plus a full-width Send row.

Verified with a separate named agent-browser session. Geometry evidence below uses the **actual production web/dist through a temporary loopback static/proxy server at 18878, forwarding API traffic to the real hosted server at 18877**. This proves current frontend behavior against the real API; it is **not proof that the hosted binary embeds these assets**. At that stage root was correcting embedded asset/build tracking; the subsequent direct 18877 result is recorded below.

| Viewport | Workspace scroll excess | Horizontal viewport overflow | Visible response code height |
| --- | --- | --- | --- |
| 1280 x 640 | 0 px | None | 115 px |
| 360 x 640 | 0 px | None | 95 px |
| 390 x 640 | 0 px | None | 95 px |
| 1360 x 768, after keyboard resize | 0 px | None | 223 px |

At 360 px width the URL field is 164 px wide and Send stays on one line. At short heights both the URL/actions and response status/body remain visible. The library separator retained focus and changed `aria-valuenow` from 35 to 30 on ArrowUp. Long response content stays in CodeMirror's own scroller.

Screenshots: `artifacts/layout-response-final-1280x640.png`, `layout-response-final-1360x768.png`, `layout-response-final-360x640.png`, `layout-response-final-390x640.png`, and `layout-post-dark-final-1360x768.png`.

The old default `https://echo.apifox.cn` returned documentation HTML after redirects despite status 200. Live requests confirmed `https://echo.apifox.com/post` returns Echo JSON, so new workspace environments now use `.com`. The POST fixture is named `发送 Echo POST 请求`, uses `/post` and a JSON body, and tests both status 200 and `/json/name == "MoleAPI"`. Both assertions passed through the actual browser/backend execution path (one observed run: 200, 308 ms, 491 B).

Axe checks of the body/response editor found and then verified fixes for missing CodeMirror textbox names/focus and low contrast in selected method labels, active navigation, badges, counters, primary actions, and one OneDark JSON property token. CodeMirror now receives labels/tabindex through `EditorView.contentAttributes`; an explicit HighlightStyle class adjusts dark JSON-property color without relying on generated selectors. Required mature CodeMirror packages are explicit dependencies. Final automated WCAG 2A/2AA scans report **zero violations in light and dark**, with one incomplete short numeric/fold-glyph contrast check remaining for manual assessment; this is not a claim of complete accessibility conformance.

Final follow-up `npm run typecheck`, `npm run test` (17 tests), and `npm run build` pass. Latest production entry at this report update: `index-1o3mOjXC.js`, `index-CNjRpOeB.css`. No backend, Rust, CI or commits were changed by this agent.


### Direct embedded binary verification

After root fixed asset production/tracking and restarted the actual hosted binary, the isolated browser navigated directly to `http://127.0.0.1:18877` and loaded `index-1o3mOjXC.js`. The page contained the new resizable `data-group` element. A saved GET to the real hosted `/api/health` returned 200 OK (1 ms, 33 B), displayed `{"status":"ok","version":"0.1.0"}` in the visible JSON editor, and showed no main-workspace scroll excess or horizontal viewport overflow at 1280 x 640. The response editor was 115 px high. Focusing the library separator and pressing ArrowUp changed its ARIA value 35 -> 30 while retaining focus, increasing the editor to 133 px.

Direct checks at 360 x 640 and 390 x 640 also found no workspace scroll excess or viewport overflow; the response editor was 111 px high after that keyboard resize. The direct 1360 x 768 check likewise showed no overflow. The direct mobile request/response scan reported zero automated WCAG 2A/2AA violations, with one incomplete check.

Current embedded-binary screenshots: `artifacts/layout-embedded-response-1280x640.png`, `layout-embedded-response-360x640.png`, `layout-embedded-response-390x640.png`, and `layout-embedded-response-1360x768.png`. These are direct hosted binary evidence, distinct from the earlier production-dist proxy checks. Root's browser session was never navigated or modified by this agent.
