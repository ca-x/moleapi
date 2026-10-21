# Database pool starvation after a monitored operation completes

## Observed failure

The shared cross-instance Mock regression intermittently returned 500 for a saved example configured to return 401. Background scheduling/notification reads also failed while the handler was stuck. Earlier diagnostic output identified `ConnectionAcquire(Timeout)`; changing Mock status handling would not resolve that pool error.

## Cause

The shared handlers pinned an operation and membership watcher, then passed `&mut` references to `tokio::select!`. The losing branch expression ended, but the pinned future itself lived until the outer handler returned. A suspended DB watcher could retain a pooled connection while the handler awaited its final membership/version read. On SQLite's one-connection pool, the final read could not proceed and the retained watcher was no longer polled.

```mermaid
sequenceDiagram
    participant H as Shared handler
    participant W as Watch future
    participant P as Single-connection pool
    H->>W: Poll beside operation
    W->>P: Acquire connection
    P-->>W: Connection
    Note over H,W: Operation completes; borrowed select returns
    H->>P: Final permission read
    Note over W,P: Watch remains alive and unpolled
    P--xH: ConnectionAcquire timeout
```

The 200 ms first-watch delay reduced the chance of overlap for fast requests. It did not constrain the losing future's lifetime. Expanding the pool or retrying reads would not establish the required resource-release order.

## Fix and boundaries

`collaboration::with_access_watch` owns the operation, watcher and final-check futures. The two raced futures are pinned in an inner block that ends before the final check is polled. All seven shared wrappers use this boundary: collection run, saved execution, draft HTTP, direct Mock, specification preview, linked Schema validation and routed Mock.

A denied watcher still has priority when both branches are ready, skips final-result release and drops the operation. The shared run watcher still cancels the original internal job ID. A completed operation returning an error still performs final authorization before returning that error.

The scheduled execution watchdog used the same retained-future pattern before `finish()` wrote its occurrence. Its runner/watchdog/select now have an inner scope too. Shutdown branch order, the two-second cancellation grace period and record/status handling are preserved.

## Evidence

- `completed_operation_releases_pending_watcher_before_final_database_read` uses one actual SQLite pooled connection. A parked watcher owns a transaction, the operation completes, and final verification runs `SELECT 1`. The original lifetime deterministically failed with `ConnectionAcquire(Timeout)` under the test's 200 ms acquisition budget; the scoped version passes for operation success and error. Production pool/timeouts are unchanged.
- `denied_watcher_releases_running_operation_and_skips_final_check` checks cancellation cleanup using the same real pool.
- `ready_watcher_has_priority_and_final_denial_withholds_private_output` preserves revocation precedence and final-result fencing.
- Full server library 22 cases plus affected member/scheduling/notification integration 41 cases pass. The original cross-instance Mock regression passes 10 consecutive runs.
- Strict server Clippy, package formatting and embedded server build pass. Independent source review found no regression in either the seven shared wrappers or scheduling scope change.

This proves the reproduced starvation mechanism and its resource-release invariant. It does not diagnose unrelated database restore 500 failures or replace current-head PostgreSQL/MySQL, Docker and installed-native acceptance gates.
