# Workspace version history and restore coverage

Scope: `versioning-006` slice specified in [workspace-version-restore.md](specs/workspace-version-restore.md). The existing owner-scoped `GET /api/workspaces/{id}/versions` lists archived whole-workspace snapshots. This batch adds a confirmed restore operation and a workspace history dialog; it does not establish per-resource version history or branch/Git workflows.

| Behavior | Evidence |
| --- | --- |
| Restore with CAS | `POST /api/workspaces/{id}/versions/{revision}/restore` selects only the owner's archived snapshot, validates it, and uses the shared `storage::replace` transaction. A successful restore writes a new revision and archives the previously current version; stale requests return 409. A bounded retry handles SQLite's verified `database is locked` write race so a competing restore reaches the CAS check. |
| Ownership and side effects | Real database API tests reject a foreign owner and missing version with 404. Restoring an older snapshot without a request revokes that request's public Mock share through the existing replacement path. |
| Interface | The dialog shows revision time, name, collection/request counts and bounded names. A second confirmation names the selected revision. Unsaved drafts enter the existing save/discard guard; a stale server revision opens the existing conflict dialog. An account switch during a pending restore never installs the returned workspace into the new account. |

Local evidence at this branch: the SQLite `database` integration test passes with cross-instance one-winner restore; the frontend focused 3-case restore suite and full 382-case suite pass, along with strict server all-target Clippy, scoped rustfmt, typecheck and production frontend/embedded server builds. An actual hosted 390px Chinese save→history→restore→reload flow and English history preview passed. Document width equaled 390px. The final browser audit reported 0 WCAG A/AA violations, with two incomplete manual checks on Radix focus guards/background and contrast in the existing navigation rail/dialog description. PostgreSQL/MySQL and fixed-commit CI remain pending.

Remaining scope: resource-level history/diff, branch and merge review, Git integration, published API version labels, history retention policy, and final platform/release gates.
