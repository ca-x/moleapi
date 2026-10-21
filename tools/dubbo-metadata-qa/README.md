# Apache Dubbo metadata report fixture

This separate fixture starts an official Apache Dubbo 3.3.6 provider with Curator's embedded ZooKeeper and a ZooKeeper `MetadataReportConfig`. The provider publishes `FullServiceDefinition` for `fixture.RegistryService`: two methods, parameter/return types, a `Profile` DTO with nested `Address`, `Tier` enum, List and Map properties, and the original JSON are read through Apache `MetadataReport.getServiceDefinition`. The test proves that metadata lookup does not execute a business method. A second digest-protected run proves anonymous and wrong-password reads fail.

Rust checks every resolved metadata-center address, gives the mature ZooKeeper SDK only a pinned literal IP, reads `/dubbo/metadata/<service>/<version>/<group>/provider/<application>` under one deadline, caps the original definition at 1 MiB, and checks its service and method outline. The driver compares the exact original JSON bytes with the Apache API result. Hosted private-IP policy must reject the same endpoint.

The product endpoint is owner-scoped. Preview shows the method outline, linked DTO/enum/collection definitions and original document without changing the workspace. Import requires a saved source request, matching SHA256, expected workspace revision, and an explicit add/skip/replace decision for each method. Existing matching methods default to skip. A linked `dubbo-metadata` specification retains the original definition, metadata endpoint and import context; default backup screens nested secrets, while explicit private export preserves the source. Known nested DTO arguments receive editable `$class`/`$` JSON drafts with null scalar/enum fields; List/Set and Map properties start empty, and cycles stop at null. Other arguments start as `null`. Metadata credentials are not copied to each generated method. The Java worker separately invokes a populated nested DTO against this provider and checks returned values.

CI builds the official Java fixture and pinned Dubbo worker, then retains JSON evidence plus Java logs. Local JRE/ECJ builds may pass `--classes` and `--dependencies` to `python3 tools/dubbo-metadata-qa/verify.py --java /absolute/java --jar /absolute/engine.jar --output /tmp/metadata-evidence.json`.

## Shared workspace integration tests

`crates/server/tests/collaboration_dubbo.rs` has offline policy/source tests and two explicitly configured real fixture tests. The source tests use checked HTTP I/O against a scripted Nacos endpoint; this is separate from the official server evidence below.

For the real tests, compile the current Java fixture classes with Java17+ and their declared Maven dependencies. Use this project's compiled classpath for `fixture.MetadataFixture --hold` (Apache/Curator), and the `../dubbo-nacos-qa/java` project's classpath for `fixture.NacosMetadataFixture --auth-namespace-hold` against the pinned authenticated Nacos2.5.1 fixture described there. Start providers/server from a temporary working directory so Derby/Nacos logs stay out of the repository. Capture each provider's `METADATA_*=` stdout lines into a JSON object without changing `METADATA_JSON` bytes. The Nacos DTO is Serializable so its generic invocation is a valid Apache wire case.

The Nacos shared test uses QA user `fixture-reader` / `fixture-password`, role `shared-dubbo-reader`, namespace `moleapi-qa` READ permission (`resource=moleapi-qa:*:*`, `action=r`). Create these through the isolated server's authenticated `/nacos/v1/auth/users`, `/nacos/v1/auth/roles` and `/nacos/v1/auth/permissions` endpoints before running. The official publisher still uses its documented fixture administrator. The reader's distinct name avoids the deliberately protected collision between private username `nacos` and the fixture application's public name; source screening must not be disabled to pass that case.

Supply explicit absolute runtime/receipt paths, then run serially so a child-PID observer cannot see another test's worker:

```bash
MOLEAPI_DUBBO_WORKER_JAR=/absolute/pinned-worker.jar \
MOLEAPI_CODEGEN_JAVA=/absolute/java/bin/java \
MOLEAPI_DUBBO_SHARED_FIXTURE=/absolute/zookeeper-fixture.json \
MOLEAPI_DUBBO_NACOS_SHARED_FIXTURE=/absolute/nacos-fixture.json \
cargo test --locked -p moleapi-server --test collaboration_dubbo -- --include-ignored --test-threads=1
```

The pinned JAR must match `vendor/dubbo-worker/manifest.json`. These explicit cases use Linux `/proc` to prove actual Java PID reap; generic offline runs leave them ignored. They cover owner/member same-ID isolation, saved/draft calls and assertions, null/exception/privacy, both registries, source/SHA/DTO/CAS, owned immediate persistence and Stop/role/revision/logout. EDAS and installed/native/platform integration have separate gates.
