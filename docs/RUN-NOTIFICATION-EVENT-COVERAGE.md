# Interactive/scenario/CI notification bindings

Ordinary owned collection/scenario execution now uses the existing notification targets, leased delivery queue, provider-specific transports and bounded retries. Saved scenarios carry additive default-empty notification_ids. A run may omit that option to inherit a selected scenario's targets, provide explicit target IDs, or provide an empty list to suppress defaults. Unique/bounded IDs and owner/workspace membership are checked before any request is sent. Unavailable references from another serving instance reject execution explicitly.

run_origin accepts interactive or ci. It supplies an event label, not additional privileges or proof that a caller is a trusted CI process. The completion envelope identifies RUN_COMPLETED or CI_RUN_COMPLETED, while scheduled jobs retain SCHEDULE_RUN_COMPLETED. All paths preserve owned delivery/report IDs. Scheduled invocation passes an explicit empty ordinary-run binding; its existing scheduler completion event is the only notification, even when the referenced scenario has defaults.

Ordinary notifications enqueue in the saved-report transaction after privacy projection and source/workspace admission. Failed persistence or stale/deleted owner/workspace cannot publish a delivery referencing an unavailable report. Failed/stopped/canceled flow outcomes are represented explicitly. Resource/origin-scoped outcome state supports the existing change-only filter. Event names use the report's runtime privacy-projected values, followed by target credential screening; bodies/logs/variable setters remain absent. CI and interactive resource dedup identities are separate from scheduled definitions.

The bilingual testing page can override default targets per run, including an explicit empty override, and the saved scenario editor records defaults through normal CAS/sync. The owned target selector displays channel availability while never requesting private URLs. Scenario references remain importable/synchronizable identifiers; target operational definitions are still local to the serving instance.

## Necessary evidence

Four real local fixtures prove inherited interactive events, explicit suppression, CI labels, linked saved report IDs, wrong-owner target rejection before HTTP execution, report persistence failure sending no event, no duplicate ordinary event for scheduled scenarios, and runtime-issued private names excluded from payloads. UI cases prove target selection, empty override options and existing scenario/runner forms; bilingual catalogs, TypeScript, production frontend and scoped Clippy passed. Only changed paths were repeated. No real third-party recipient was contacted.

Workspace collaboration subscriptions, executable CI/runner CLI integration, additional providers, vendor installation/auth verification, distributed runner enrollment and all other unfinished matrix requirements remain open before unified review/main merge/cleanup and release.
