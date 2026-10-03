# GraphiQL dependency patch

`@graphiql/react` is pinned to 0.39.1. `npm ci` applies the patch using
`patch-package` through `postinstall`, failing if the patch no longer applies.

The upstream OperationEditor installs a debounced Monaco model callback once.
Its operation-facts helper retains the operation selection and schema from the
initial mount, even after the public selection action or saved-schema restoration.
Editing a multi-operation document therefore resets a saved/chosen second
operation to the first. The patch reads these existing inputs through a current
React ref. It does not replace the parser, editor or selection algorithm.

Verification: restore a two-operation document with the second selected; edit a
comment; select the first; edit another comment; confirm both selection and actual
execution remain on the selected operation. Also recheck schema navigation,
Ctrl/Cmd+Enter and immediate Send after editing. Remove this patch once an upstream
release preserves current state in its mounted editor callbacks.

The tab accessibility patch moves `role="tab"` and `aria-selected` from the
non-focusable wrapper to its actual tab button, leaving the close button as a
separate control. GraphiQL is pinned to 5.4.0 for this patch.
