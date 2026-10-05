# Independent inheritance frontend review

Reviewed uncommitted frontend inheritance/folder changes against base `81ac1a0` and `docs/specs/authentication-inheritance.md`. Applied the Emil design engineering review format. No source implementation edits, Docker, delegation or root browser-process changes. Full auth/parent-goal coverage is outside this slice.

## Important findings

| Before | After | Why |
| --- | --- | --- |
| **P1:** `CollectionSettingsDialog.tsx` edits collection variables through raw `PairEditor` and replaces metadata on Apply without `state.localVariables.reconcile`. Browser-local overrides still reference old names after rename/delete. | Reconcile old/latest collection variable rows only on successful Apply after owner/CAS/tree validation, preserving Cancel and checking persistence failure. Cover rename, deletion and descendant execution values. | Existing `VariableScopeEditor` reconciles browser buckets. `executionLocals` deliberately includes bucket entries absent from current rows, so removing a private `token` row in the new dialog leaves it active as a ghost variable for descendants; renaming loses the intended local override on its new key. Local source/privacy semantics must remain consistent across editors. |
| **P2:** `authentication/inheritedSource.ts` picks the first saved parent whose literal `auth.kind !== 'inherit'`, with no effective variable context; RequestEditor presents that as a definite source. | Resolve selector templates against actual effective/local variables without signing, or show an explicitly execution-dependent/unresolved preview whenever such a selector affects the chain. Preserve original keys/claims/modes. | Core inheritance resolves each selected mode before choosing a source. A leaf with `kind:'{{mode}}'`, environment `mode:'inherit'`, and root Bearer is previewed as leaf while execution uses root. Missing selectors similarly display a definite source even though execution rejects them. The preview must not guess the source of credentials. |

Both findings are static source/call-path findings; no actual browser reproduction is claimed. Reports were sent to root. Existing dialog tests cover concurrent request edits/CAS but not local-bucket reconciliation; current source hint has no focused regression tests.

## Boundaries checked

Tree helpers preserve descendant identities and deterministic source order, reject parent cycles/missing parents on moves and enforce depth16. Subtree deletion preserves siblings. New requests explicitly default to inherit without copying parent credentials. Existing explicit No Auth remains a stop. Ancestor execution-local merging permits leaf shared values to shadow parent locals while retaining unrelated ancestor values; browser/native paths are separated.

Collection settings clone editable source until Apply. Owner/workspace checks precede mutation; metadata CAS excludes request contents and merges current requests so concurrent request edits survive. Cancel does not persist metadata. Radix owns modal containment; closing restores focus to the collection menu trigger when present. Locale rendering does not reconstruct source values. Sidebar collapse state resets on owner/workspace changes and filtering expands traversal. No new query/navigation animation was introduced.

## Fresh verification

- `npm --prefix web test -- src/features/collections src/features/variables/localValues.test.ts --reporter=dot`: **13 passed across3 files**, exit0.
- `npm --prefix web run typecheck`: exit0.
- Root-reported full158-test/build gates were read but not independently rerun in this review. Actual nested-folder settings/move/delete/390px/light-dark browser QA remains root-owned.

Fix the two findings and add focused regression evidence before frontend approval. No native-platform or full-auth approval is implied.

## Focused fix re-review

**Both findings are resolved; scoped frontend review approved.**

| Before | After | Why |
| --- | --- | --- |
| Folder variable edits could leave ghost browser-local overrides after rename/delete. | Successful Apply now reconciles the current collection rows only after owner/CAS/tree validation and before source commit. Failed browser persistence raises a localized error and keeps both source and pending draft; Cancel never reconciles. | Reuses the existing row-ID-aware local reconciliation and preserves private source semantics across both variable editors. The actual Radix rename/storage-failure regression exercises the Apply ordering. |
| A templated auth selector displayed a guessed concrete parent source. | The nearest relevant templated parent/workspace selector produces a dynamic source hint, rendered as determined after execution-time variable resolution. Concrete stopping selectors still show their real source. | Keeps source preview honest without resolving secrets, signing or rewriting the original auth draft. Helper regression covers dynamic leaf versus concrete ancestor. |

Fresh independent focused verification: **15 tests passed across4 files**, exit0, plus typecheck exit0. No definite important issue remains within this fix scope. Root owns final embedded browser/rebuild/full-suite gates; native/full-auth/parent-goal completion remains outside this approval.

### Source-only imported folder-variable flag

**Scoped follow-up approved; prior fixes remain approved.** No definite important issue found in the new helper/dialog flag.

| Before | After | Why |
| --- | --- | --- |
| Imported folder variable definitions could imply execution semantics beyond actual Postman runtime behavior. | `variables_enabled:false` retains definitions, exposes an explicit bilingual execution checkbox/source-only hint, and is omitted from execution-local ancestry. Native absent/null flags remain enabled by default. | Prevents retained browser/native local overrides from activating imported source-only nodes; explicit user opt-in changes execution without erasing definitions or affecting dialog Cancel/Apply scope fences. |

Fresh independent focused collections/local-values run passed **15 tests across3 files**, exit0, including the retained-browser-override regression. Root-reported full161-test/build evidence was noted, not independently rerun here. This is frontend flag/helper/dialog approval; Postman interchange/runtime proof remains root/backend-owned.

### STDIO opt-out and variable labels

**Two-change scoped follow-up approved.** No definite important issue found; prior approvals remain intact.

| Before | After | Why |
| --- | --- | --- |
| STDIO hid the entire auth tab, preventing a request from opting out of inherited workspace HTTP auth. | STDIO now retains the auth tab with inherit/No Auth options; Basic/Bearer choices are excluded through `credentials:false`, while API Key/JWT/Digest remain ineligible. Headers remain hidden. | Gives users an explicit source-preserving way to stop incompatible inheritance; unsupported existing drafts can still be corrected instead of having credentials silently ignored. |
| Folder variable pairs used generic name/value labels. | Inputs explicitly say Variable name / Shared value using existing bilingual labels. | Clarifies that edited values are shared source, preserving the separate private-local override semantics. |

Fresh independent focused authentication/collections tests passed **12 across5 files**, exit0. Root-reported full161-test/build gates and actual STDIO/backend behavior remain separate evidence; no native-platform/full-auth claim is implied.
