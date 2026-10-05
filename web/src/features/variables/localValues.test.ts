import { describe, it, expect } from "vitest";
import {
  assignLocal,
  bucketKey,
  cleanBuckets,
  executionLocals,
  reconcileLocalRows,
} from "./localValues";
import { initialData } from "../../shared/model";
import type { Workspace } from "../../shared/types";
const workspace = (): Workspace => ({
  id: "workspace",
  name: "test",
  revision: 1,
  updated_at: "now",
  data: initialData(),
});
describe("private local variable boundaries", () => {
  it("only sends selected scope local values, preserves explicit empty overrides and omits disabled variables", () => {
    const source = workspace();
    source.data.global_variables = [
      { id: "global", key: "shared", value: "server-value", enabled: true },
    ];
    const env = source.data.environments[0];
    env.variables.push({
      id: "disabled",
      key: "disabled",
      value: "shared",
      enabled: false,
    });
    let buckets = assignLocal({}, "project", "", "shared", "");
    buckets = assignLocal(
      buckets,
      "environment",
      env.id,
      "disabled",
      "private",
    );
    buckets = assignLocal(
      buckets,
      "environment",
      "other-env",
      "wrong",
      "other-private",
    );
    const locals = executionLocals(
      source,
      source.data.collections[0].id,
      env.id,
      buckets,
      false,
    );
    expect(locals).toEqual([{ scope: "project", key: "shared", value: "" }]);
    expect(source.data.global_variables[0].value).toBe("server-value");
  });
  it("native stored values are local execution overrides and can be unlinked without deleting shared values", () => {
    const source = workspace();
    source.data.environments[0].variables[0].local_value = "native-local";
    const env = source.data.environments[0];
    expect(executionLocals(source, undefined, env.id, {}, true)).toEqual([
      {
        scope: "environment",
        key: env.variables[0].key,
        value: "native-local",
      },
    ]);
    const buckets = assignLocal({}, "environment", env.id, "key", "private");
    const cleared = assignLocal(
      buckets,
      "environment",
      env.id,
      "key",
      undefined,
    );
    expect(cleared[bucketKey("environment", env.id)]).toEqual({});
  });
  it("scope buckets do not treat prototype names as executable object properties", () => {
    const buckets = assignLocal({}, "project", "", "__proto__", "safe-value");
    expect(Object.getPrototypeOf(buckets[bucketKey("project")])).toBe(
      Object.prototype,
    );
    expect(Object.hasOwn(buckets[bucketKey("project")], "__proto__")).toBe(
      true,
    );
    const cleaned = cleanBuckets(JSON.parse(JSON.stringify(buckets)));
    expect(cleaned[bucketKey("project")]["__proto__"]).toBe("safe-value");
  });
});

it("moves a renamed variable local override and clears deleted rows without changing other scopes", () => {
  const before = [
    { id: "a", key: "old", value: "shared", enabled: true },
    { id: "b", key: "removed", value: "", enabled: true },
  ];
  let buckets = assignLocal({}, "project", "", "old", "private");
  buckets = assignLocal(buckets, "project", "", "removed", "deleted");
  buckets = assignLocal(buckets, "environment", "other", "old", "unrelated");
  const next = reconcileLocalRows(buckets, "project", "", before, [
    { ...before[0], key: "new" },
  ]);
  expect(next[bucketKey("project")]).toEqual({ new: "private" });
  expect(next[bucketKey("environment", "other")]).toEqual({ old: "unrelated" });
});

it("preserves private overrides through a temporarily empty name and drops them on row deletion", () => {
  const row = { id: "row", key: "token", value: "shared", enabled: true };
  const buckets = assignLocal({}, "project", "", "token", "private");
  const empty = reconcileLocalRows(
    buckets,
    "project",
    "",
    [row],
    [{ ...row, key: "" }],
  );
  expect(executionLocals(workspace(), undefined, null, empty, false)).toEqual(
    [],
  );
  const renamed = reconcileLocalRows(
    empty,
    "project",
    "",
    [{ ...row, key: "" }],
    [{ ...row, key: "renamed_token" }],
  );
  expect(renamed[bucketKey("project")]).toEqual({ renamed_token: "private" });
  const deleted = reconcileLocalRows(
    empty,
    "project",
    "",
    [{ ...row, key: "" }],
    [],
  );
  expect(JSON.stringify(deleted)).not.toContain("private");
});

it("switching development and production selects independent overrides for the same variable", () => {
  const source = workspace();
  source.data.environments = [
    {id: "dev", name: "开发", variables: [{id: "d", key: "token", value: "dev-shared", enabled: true}]},
    {id: "prod", name: "生产", variables: [{id: "p", key: "token", value: "prod-shared", enabled: true}]},
  ];
  let buckets = assignLocal({}, "environment", "dev", "token", "dev-login");
  buckets = assignLocal(buckets, "environment", "prod", "token", "prod-login");
  expect(executionLocals(source, undefined, "dev", buckets, false)).toEqual([
    {scope: "environment", key: "token", value: "dev-login"},
  ]);
  expect(executionLocals(source, undefined, "prod", buckets, false)).toEqual([
    {scope: "environment", key: "token", value: "prod-login"},
  ]);
  expect(executionLocals(source, undefined, null, buckets, false)).toEqual([]);
  expect(source.data.environments.map((env) => env.variables[0].value)).toEqual(["dev-shared", "prod-shared"]);
});

it("merges inherited browser local values while a child shared declaration shadows the parent local",()=>{
 const w=workspace();const root=w.data.collections[0];root.id="root";root.variables=[{id:"one",key:"token",value:"root-shared",enabled:true},{id:"two",key:"root-only",value:"root",enabled:true}];
 const child={id:"leaf",parent_id:"root",name:"Leaf",description:"",variables:[{id:"leaf-token",key:"token",value:"leaf-shared",enabled:true}],requests:[]};w.data.collections.push(child);
 const buckets={[bucketKey("collection","root")]:{token:"root-private", "root-only":"root-local"}};
 const values=executionLocals(w,"leaf",null,buckets,false);
 expect(values.find(v=>v.key==="token")).toBeUndefined();expect(values.find(v=>v.key==="root-only")?.value).toBe("root-local");
 const override=executionLocals(w,"leaf",null,{...buckets,[bucketKey("collection","leaf")]:{token:"leaf-local"}},false);
 expect(override.find(v=>v.key==="token")?.value).toBe("leaf-local");
});

it("source-only imported folder variables cannot activate retained browser overrides",()=>{
 const w=workspace();const c=w.data.collections[0];c.variables_enabled=false;c.variables=[{id:"x",key:"token",value:"source",enabled:true}];
 expect(executionLocals(w,c.id,null,{[bucketKey("collection",c.id)]:{token:"retained-private"}},false)).toEqual([]);
});
