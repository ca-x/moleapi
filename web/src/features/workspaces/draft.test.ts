import { describe, expect, it } from "vitest";
import { initialData } from "../../shared/model";
import type { Workspace } from "../../shared/types";
import { acknowledgeSave, hasChanges, retainLocal } from "./draft";
const workspace = (): Workspace => ({
  id: "one",
  name: "Saved",
  revision: 1,
  updated_at: "2026-10-02",
  data: initialData(),
});
describe("workspace draft lifecycle", () => {
  it("acknowledges the saved snapshot while preserving edits made during the save", () => {
    const saved = workspace();
    const sent = { ...saved, name: "Sent to server" };
    const draft = {
      ...sent,
      name: "Typed while saving",
      data: { ...sent.data, active_environment_id: null },
    };
    const server = { ...sent, revision: 2 };
    const result = acknowledgeSave({ draft, saved }, server);
    expect(result.draft?.name).toBe("Typed while saving");
    expect(result.draft?.data.active_environment_id).toBe(null);
    expect(result.draft?.revision).toBe(2);
    expect(result.saved?.name).toBe("Sent to server");
    expect(hasChanges(result)).toBe(true);
  });
  it("ignores save acknowledgments for a previously selected workspace", () => {
    const saved = workspace();
    const next = { ...saved, id: "two", name: "Another workspace" };
    const current = { draft: next, saved: next };
    expect(acknowledgeSave(current, { ...saved, revision: 2 })).toBe(current);
  });
  it("keeps local collections, secrets and spec sources when rebasing after a CAS conflict", () => {
    const saved = workspace();
    const draft = structuredClone(saved);
    draft.data.environments[0].variables[1].value = "local-secret";
    draft.data.specifications = [
      {
        id: "spec",
        name: "API",
        kind: "openapi",
        dialect: "3.1.0",
        source: "openapi: 3.1.0",
      },
    ];
    draft.data.collections[0].requests[0].body = "unsaved body";
    const remote = { ...saved, name: "Remote", revision: 4 };
    const result = retainLocal({ draft, saved }, remote);
    expect(result.draft?.data).toEqual(draft.data);
    expect(result.draft?.revision).toBe(4);
    expect(result.saved).toBe(remote);
    expect(hasChanges(result)).toBe(true);
  });
});

it("equivalent server key ordering does not keep a successfully saved draft dirty",()=>{
  const original=workspace();
  function reordered(value:unknown):unknown {if(Array.isArray(value))return value.map(reordered);if(value&&typeof value==="object")return Object.fromEntries(Object.entries(value).reverse().map(([key,item])=>[key,reordered(item)]));return value;}
  const server={...original,data:reordered(original.data) as Workspace["data"],revision:2};
  expect(server.data).toEqual(original.data);
  const result=acknowledgeSave({draft:original,saved:original},server);
  expect(hasChanges(result)).toBe(false);
});
