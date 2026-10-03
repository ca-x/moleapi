# SOAP / WSDL Client Implementation Plan

> Agentic workers: subagent-driven-development, one fresh Rust implementer and independent backend/UI reviews; root frontend/formats/docs. Already authorized scope.

Goal: actual SOAP1.1/1.2 requests with runtime WSDL/XSD operation/template tooling and shared finite HTTP scripts/response.
Architecture: mature WSDL/XML/XSD models in core; owner source/import/schema/template API; existing checked HTTP pipeline extended SOAP metadata/Fault; lazy XML workbench and canonical interchange.
Spec: docs/specs/soap-client.md.

## Constraints and review focus

Mature parser/DOM/writer/header libraries only; no custom protocol grammar; original definitions retained, private scoped metadata isolated; node/depth/source/template bounds; DTD/external imports disabled or explicitly checked. Features first/no localDocker/packaging. Mainfinal after preserveuntracked originals/mergedbranchcleanup.
Review: namespace/action/version mismatches; envelope edited during async schema/template operation; Fault200 vsHTTP status; recursive source/DTD/import handling; sensitive XML payload/labels defaultexport; pre/post scripts that mutate actual transport body.

## Task 1 — Rust modules and service fixture

- [x] Verify wsdl/roxmltree/quick-xml/xmltree/XSD capability and exact contract; report DTO/API early.
- [x] Add canonical source bundle/model/schema/template with bounded mature parser and original source preservation; incomplete draft validation separated.
- [x] Integrate actual SOAP HTTP envelope/version/action/content-type and post-edit validation into finite request pipeline; structured SOAP Fault alongside real Response.
- [x] Add owner import/schema/template endpoints and source/redirect/TLS/auth/scopes/privacy behavior, XML-aware default/history handling.
- [x] Real SOAP1.1/1.2/WSDL/XSD/auth/Fault/script/malformed/source restore fixtures and tests; strict checks/report/sourcefreeze for review.

## Task 2 — Dedicated UI/interchange and actual QA

- [x] Root lazy mature XML workbench, source dialog/service/operation/template/draft/settings/auth/script/response/Fault view.
- [x] Native original source/draft roundtrip and safe default credentials; reject unrepresentable foreign exports. Meaningful UI lifecycle/async edit/navigation/schema/owner tests.
- [x] Actual browser fixture flows, independent reviews, fixes, final source/API/UI tests and commit/proxy push. Continue MCP/A2A/TCP/Dubbo/Webhook/Data and shared extraction/testing/collaboration full matrix. Build final integration only after features as user instructed.

SOAP source/API/UI acceptance complete with independent review; commit/proxy push tracked in application ledger. Continuing MCP and the full matrix is separate ongoing work; builds remain deferred.
