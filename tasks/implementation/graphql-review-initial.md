# Initial independent GraphQL correctness findings

Independent reviewer graphql_review read current source and reproduced two issues using isolated local artifacts, without altering product source or the root QA service. Its final turn ended before writing the ordinary report; root preserves the findings here for the fix loop.

1. SDL conversion does not preserve root semantics when introspection has queryType Query and null mutationType/subscriptionType, but another ordinary object is named Mutation. Generated Cynic SDL omits an explicit schema declaration; GraphQL-JS buildSchema assigns Mutation as an operation root. Require explicit preservation of the actual root map, verified against mature introspection/model output.
2. Recursive SDL parsing has no structural depth guard despite a byte cap. A small malformed nested type input caused process stack failure in an isolated local core-library check. Apply a mature lexer/parser depth option or parsing containment before recursive AST processing, and cover the related query/default-value parsing paths. This is a defensive robustness fix, not a public service test.

Optional subscription_url credential capture was under inspection at handoff; no final finding/evidence was delivered for it. The original reviewer verified the vendor patch matches documented observer/error-terminal/normal-close corrections and retains the upstream engine/parser/license. Root assigned both concrete findings and the credential-field audit to the implementer as fix round1. No completion claim until independent follow-up and covering verification.
