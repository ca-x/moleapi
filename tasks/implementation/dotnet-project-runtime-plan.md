# .NET Project Runtime Plan

Parent spec: `docs/specs/generated-project-runtime-fixtures.md`.

1. Extend trusted fixture generation to the default C# generichost client and ASP.NET Core target for3.0/3.1. Add explicit .NET executable selection to the QA harness.
2. Author consumer build/call code using generated hosting/token/response APIs, and server build/start/route checks. Keep original generated files unchanged; consumer authentication setup belongs only in a separate fixture host if the skeleton requires it. Test method/path/query/header/Bearer/Unicode JSON/enum/nullable/errors through the shared loopback fixture.
3. Execute the completed drivers, diagnose concrete failures and rerun affected cases. Record defaults, actual framework/tool/dependency versions and consumer implementation limits. Run generation/lint/format gates, then commit/merge/push under existing authorization.

Implementation finding: upstream ASP.NET Core Startup omitted authentication/authorization middleware, causing protected routes to fail500. Added a pinned Apache-2.0 template compatibility copy, filling only absent explicit template files; original JAR stays unchanged. A separate consumer host configures a synthetic authentication scheme to validate actual generated-route200/401 behavior without altering source.

Verification: default C# and ASP.NET Core targets passed all4 dialect/runtime cases; generation regression55 passed/7 configured external-tool cases ignored, with1 explicit Startup override regression additionally passed; scoped strict Clippy and format/diff gates passed. Real schemes/business handlers and other ecosystems/advanced keywords/platform packaging remain open.
