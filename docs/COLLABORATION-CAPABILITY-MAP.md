# Workspace collaboration capability map

Authority: `collaboration-002` through `collaboration-004` in `FEATURE-MATRIX.md`. This map separates account membership, authorization and client behavior so sharing a workspace never relies on a copied workspace document.

| Module ID | Responsibility | Depends on |
| --- | --- | --- |
| `workspace-membership` | Hosted invitation creation/acceptance/revocation, indexed account-to-workspace membership, owner/admin/editor/viewer roles | Hosted accounts, canonical workspace storage |
| `workspace-access` | Resolve actor and canonical owner, apply deny-by-default route permissions, fence in-flight work after role/revision changes | `workspace-membership` |
| `member-workbench` | Shared workspace list, role-aware bilingual controls, safe viewer projection and current-account fences in web/native clients | `workspace-access` |
| `collaboration-audit` | Record actor, owner, resource, action and result for member/role changes and shared writes without secret payloads | `workspace-access` |

Build order: `workspace-membership` → `workspace-access` → `member-workbench` → `collaboration-audit`. The first release gate covers all four together: an accepted invitation must lead to useful, correctly limited workspace access. Organization/group policy, SSO/SCIM, comments, presence and broad resource-level custom roles stay in their own matrix rows.

Current audit checkpoint: invitation, membership and shared workspace writes append actor-labelled success events transactionally; owner/admin hosted sessions can read the latest 100. Shared collection runs, canonical saved-request execution and saved Mock preview record the actor at admission and poll membership across instances while running. Editors/admins can read saved history, reports and Mock calls with a final membership check. Ad hoc request execution, public Mock share management and other shared resource actions remain closed to members, so their future audit entries and cancellation fences are still part of this release gate.

Current client checkpoint: hosted settings can create one-time invitation links, accept them under the active session and manage members. The picker shows owner and role; a dedicated shared view gives viewers a safe read-only projection and editors CAS editing for saved HTTP request settings and basic Mock examples, collection runs, saved-request/Mock previews, history, reports and Mock call records. Dedicated non-HTTP and advanced Mock/resource editor parity, account cache audit and native hosted sync remain open, so this does not complete `member-workbench`.
