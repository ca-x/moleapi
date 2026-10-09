# Script-directed collection flow

Implements the Postman flow API within the required testing/scenario scope. Reuse the existing QuickJS worker and sequential runner; do not add another scripting engine or a custom expression parser. Declarative visual scenario graphs/parallel blocks remain a separate required module.

pm.execution.setNextRequest(string) selects a request by exact ID first, or a unique name, within the selected collection subtree. Null ends the current iteration; subsequent configured iterations continue, matching Postman. Unset follows normal depth-first order. The latest successful script phase directive wins. Pre/post phases can set it; outside a collection run it does not trigger additional execution. Missing/ambiguous targets stop with an explicit report reason, without echoing untrusted/private target strings.

pm.execution.skipRequest is pre-request only. It halts remaining phase scripts, skips network/auth/token actions and post-response scripts, and returns a marked skipped result with successful pre-script variable/test/log effects. It does not count as a passed network request or create normal response history. The UI labels skip distinctly from HTTP status0. No control directive is persisted in canonical data/history.

Request order/jump indices are execution state, not workspace mutations. Across jumps, collection variables are rebuilt from ancestry plus run overlays as before. Dataset iteration metadata and values remain correct. All executed/skipped steps count toward the existing1000-step bound; existing300s deadline, job/owner cancellation and response/privacy limits remain. Loops cannot bypass quotas. Invalid targets do not silently become successful termination.

Checks: worker typed controls, last directive, invalid arguments and real skip short-circuit; actual jump/loop/null/skip with loopback requests, no skipped network/history, variable propagation, iteration/collection ownership and quota termination; frontend marked skip rendering and existing runner regressions. User instruction remains implementation first, unified review only after all remaining functions are complete.
