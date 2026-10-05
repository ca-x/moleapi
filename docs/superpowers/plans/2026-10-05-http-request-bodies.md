# HTTP body implementation plan

1. Typed source module/parser/validation and independent field interpolation in core; source keeps opaque bytes exact. Verify limits/missing vs empty files/quoted variable values.
2. Bounded reqwest multipart/binary snapshot integrated into checked transport; real Axum Multipart/byte fixtures and redirects.
3. Formats privacy plus source roundtrip, external format adapters with explicit missing-file/unsupported behavior.
4. Shared bounded file picker and modular body editor; existing RequestEditor integration and en/zh catalogs. Verify deferred selection and actual transmitted bodies.
5. Scripts/snippets representability checks, source gates, independent review, embedded browser QA; update status and push via authorized proxy. Existing CI supplies database and full build evidence. Full project objective stays unfinished.
