# MoleAPI design system
Sources: ui-ux-pro-max searches `developer API desktop workspace` and `productivity dashboard minimal` (2026-10-02); emil-design-eng; animate. Verified relevant matches: Minimalism/Swiss and Flat Design; developer typography; dense workspace spacing. Search landing page recommendations do not match this application and are not applied.

- A functional workbench with a narrow navigation rail, collection sidebar, editor tabs and a response pane. No marketing hero, ornamental stats or decorative gradients.
- Radix UI Themes supplies neutral slate tokens and accessible teal actions. Light default, dark selectable. Text retains 4.5:1 contrast; states use labels as well as color.
- Bundle IBM Plex Sans and JetBrains Mono from fontsource; system Chinese fallback. Main controls 14–16px, code 13px, clear 8px spacing rhythm. Monospace for HTTP methods, URLs and responses only.
- Radix primitives own focus trapping, menus, selects, dialogs and tooltips. Lucide supplies icons; visible labels on primary actions. Every icon-only control has a name. Toolbar actions use tooltips.
- Request/tab/list selection and keyboard-triggered changes are instant. Pointer button press: scale(.97), 160ms cubic-bezier(.23,1,.32,1). Dialogs: 200ms opacity/scale(.96); exits 150ms. Reduced motion removes movement. Hover changes gated to fine pointers. No transition:all, no layout-property animation.
- Sidebar hides behind a labeled toggle under 860px; request toolbar wraps and panels scroll independently, never the viewport horizontally. Touch targets at least 44px on coarse pointers.
- Persist explicit saved revisions; preserve unsaved edits if a request fails. Loading buttons and inline errors. Sync conflict requires explicit resolution and has an export affordance.
