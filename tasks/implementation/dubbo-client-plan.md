# Classic Dubbo implementation plan

Parent: remaining-work-plan milestone 3 and docs/specs/dubbo-client.md.

1. Build an isolated wire probe against an actual Apache Dubbo Java provider using scalar/DTO/list/multiple arguments, null and exception. Compare the candidate `dubbo-rs-protocol-dubbo 0.1.0` request/response bytes and parser behavior with the provider. Record dependency and tool hashes. Do not add a product menu item from compile-only evidence.
2. Select the smallest mature interoperable engine. The Rust candidate needs an injected checked socket, pre-allocation frame limit, correct typed Hessian values and cancellation; validate any dependency patch separately. If it cannot meet these without rewriting its protocol core, use a pinned official Java client in an isolated bundled-JRE worker with bounded IPC and cancellation.
3. Add a versioned Dubbo request/source model and core validation, then implement direct invocation through the shared owner/network/execution/privacy interfaces. Unsupported registry/Triple selections reject explicitly. Run real wire and malformed-input cases before expanding.
4. Add a dedicated bilingual workbench for endpoint/service/method/version/group/ordered typed arguments/attachments and response/exception display. Verify save/restore, source fences and real 390px interaction.
5. Add ZooKeeper/Nacos/EDAS provider discovery and metadata import as separate slices, each with real provider fixtures and credentials/owner/network gates. Update protocol coverage only with actual passed combinations.

Current evidence: official `dubbo 0.4.0` is Triple only. Apache Dubbo Java3.3.6 source confirms the community classic crate's single-list/binary argument body disagrees with the provider's per-typed-object read; the crate also exposes an unchecked direct socket and unbounded peer length allocation. An isolated locked Rust probe, official Java provider/consumer and manual Actions wire gate are prepared. The actual Java runtime outcome and connector/engine decision in step1 are still open. The per-request Cookie Jar slice was completed first under the user's non-generation priority.
