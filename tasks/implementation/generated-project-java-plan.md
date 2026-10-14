# Java Client and Spring Runtime Plan

Parent: `docs/specs/generated-project-runtime-fixtures.md`.

1. Extend trusted fixture generation to default Java okhttp-gson SDK and Spring spring-boot skeleton for3.0/3.1. Use explicit isolated JDK/Maven executables in QA; keep product runtime dependencies unchanged.
2. Author Maven consumer compile/call code using generated ApiClient/DefaultApi/model/error APIs and a Spring executable build/start/route check. Keep generated files unchanged except necessary pinned compatibility templates if actual defects are found. Preserve real GET/POST/auth/path/query/header/nullable/enum/Unicode/error assertions.
3. Run finished cases, diagnose concrete failures and fix the mature integration without changing library defaults or weakening contracts. Record exact tool/library/dependency/dialect evidence, run generation/lint/format gates, then commit/merge/push verified work. Other language/framework/advanced keyword/file/platform gates remain open.

Verification: all4 default Java/Spring dialect cases passed. Java compiled and called the core auth/query/header/JSON/nullable/enum/error fixture; Spring built/started and served generated OpenAPI/routes plus declared501 placeholder. Temurin JDK21.0.12.1+1/Maven3.9.11 archives were hash-verified in an isolated QA directory; emitted library/POM defaults stayed unchanged. Generation65 and strict scoped Clippy/format/diff gates passed. Extended Java profiles, other languages/frameworks, business/security and advanced/platform gates remain open.
