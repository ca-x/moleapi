import {expect,it} from "vitest";
import {initialData} from "../../shared/model";
import {inheritedAuthSource} from "./inheritedSource";
import type {Auth} from "../../shared/types";
it("does not guess a parent source when an authentication selector is templated",()=>{
 const d=initialData();const root=d.collections[0];root.auth={kind:"bearer",token:"root",username:"",password:""};
 const r=structuredClone(root.requests[0]);r.id="leaf-request";r.auth.kind="inherit";
 d.collections.push({id:"leaf",parent_id:root.id,name:"Leaf",description:"",requests:[r],auth:{kind:"{{mode}}" as Auth["kind"],token:"",username:"",password:""}});
 expect(inheritedAuthSource(d,r).scope).toBe("dynamic");d.collections[1].auth=undefined;
 expect(inheritedAuthSource(d,r)).toEqual({scope:"collection",name:root.name});
});
