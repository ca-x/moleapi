import { fingerprint } from "../../shared/model";
import type { Workspace } from "../../shared/types";

export interface DraftState {
  draft: Workspace | null;
  saved: Workspace | null;
}
export function hasChanges(state: DraftState): boolean {
  return (
    !!state.draft &&
    (!state.saved || fingerprint(state.draft) !== fingerprint(state.saved))
  );
}
/** Acknowledge only the sent snapshot; edits made while saving remain a dirty draft. */
export function acknowledgeSave(
  state: DraftState,
  server: Workspace,
): DraftState {
  if (state.draft?.id !== server.id) return state;
  return {
    saved: server,
    draft: { ...server, name: state.draft.name, data: state.draft.data },
  };
}
/** Explicit conflict retry keeps every local edit and adopts only the remote CAS revision. */
export function retainLocal(state: DraftState, remote: Workspace): DraftState {
  if (state.draft?.id !== remote.id) return state;
  return {
    saved: remote,
    draft: {
      ...state.draft,
      revision: remote.revision,
      updated_at: remote.updated_at,
    },
  };
}
