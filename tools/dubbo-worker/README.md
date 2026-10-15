# Official classic Dubbo worker candidate

`Worker.java` invokes the official Apache Dubbo3.3.6 `GenericService` once per process. It accepts at most1 MiB of JSON on stdin and emits at most1 MiB of JSON on stdout. The Rust host must supply a checked literal IP endpoint and kill the process on cancellation/deadline; this worker independently refuses hostnames. Its Maven build pins Dubbo, Netty, Fastjson2, Guava and logging dependencies and creates a shaded JAR for the bundled Java21 runtime. No runtime download or user-provided classpath is permitted.

The current independent QA calls it against an actual Apache Java provider for string, multiple arguments, exact 64-bit integer, typed DTO wrapper, list, null and remote exception. It also checks hostname refusal. The product transport, host-side source validation, address admission, bounded IPC, privacy projection, packaging on every OS and UI are not integrated yet. A passing Maven build alone cannot promote this candidate to product support.

Build in the manual `Classic Dubbo interoperability QA` workflow. The official generic client is checked before the worker, and `worker-evidence.json` records the shaded JAR hash and actual result cases. The worker's own process-level one-shot design keeps each request's credentials and class metadata from leaking into a reused JVM.
