# Default Java Extended SDK Runtime Plan

Parent: `docs/specs/generated-project-runtime-fixtures.md` and the binary/multipart/union plans.

1. Extend the default Java okhttp-gson trusted consumer with binary File upload/download, multipart File/text/required-optional-exploded/non-exploded arrays, and referenced tagged Cat/Dog union model roundtrips for3.0/3.1.
2. Use the same wire contracts and actual emitted signatures/API classes as other clients; keep generated sources unchanged and finish consumer code before checks. Explicit isolated JDK/Maven tools are already prepared.
3. Run all6 profile/dialect cases, diagnose concrete failures and repair mature engine integration without switching libraries or weakening assertions. Record exact scope/defaults/remaining limits, run generation/lint/format gates, then commit/merge/push verified work.

Findings/fixes: the default Java multipart transport loop sent its entire list at every iteration, and the API lost non-multi collection formatting. Pinned Apache-2.0 upstream templates now pass the current element and reuse the existing collection joining helper. Default library, mature models/transport and explicit override priority remain intact.

Verification:6 Java extended profile/dialect cases passed (32 HTTP requests);4 regenerated binary/union regression cases passed after the fix; generation65 passed/12 configured external-tool cases ignored;7 explicit template rendering/override cases passed; strict scoped lint/format/diff gates passed. Broader multipart/file/union/keyword and ecosystem/browser/platform scope remains open.
