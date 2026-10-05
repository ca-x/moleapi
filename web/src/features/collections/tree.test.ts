import {expect,it} from "vitest";
import {initialData} from "../../shared/model";
import {collectionAncestors,collectionDescendants,collectionRows,moveCollection,removeCollectionTree} from "./tree";
function data(){const d=initialData();const root=d.collections[0];root.id="root";d.collections=[{id:"leaf",parent_id:"folder",name:"Leaf",description:"",requests:[]},root,{id:"folder",parent_id:"root",name:"Folder",description:"",requests:[]},{id:"other",name:"Other",description:"",requests:[]}];return d;}
it("renders actual hierarchy in deterministic order and hides descendants on collapse",()=>{
 const d=data();expect(collectionRows(d.collections).map(r=>[r.collection.id,r.depth])).toEqual([["root",0],["folder",1],["leaf",2],["other",0]]);
 expect(collectionRows(d.collections,new Set(["folder"])).map(r=>r.collection.id)).toEqual(["root","folder","other"]);
 expect(collectionAncestors(d.collections,"leaf").map(c=>c.id)).toEqual(["root","folder","leaf"]);
});
it("moves complete subtrees without changing child identities and rejects cycles",()=>{
 const d=data();const next=moveCollection(d,"folder","other");expect(collectionAncestors(next.collections,"leaf").map(c=>c.id)).toEqual(["other","folder","leaf"]);
 expect(()=>moveCollection(d,"root","leaf")).toThrow();expect(()=>moveCollection(d,"folder","missing")).toThrow();
 expect(d.collections.find(c=>c.id==="folder")?.parent_id).toBe("root");
});
it("removes all descendants and preserves sibling collections",()=>{
 const d=data();expect([...collectionDescendants(d.collections,"folder")].sort()).toEqual(["folder","leaf"]);
 expect(removeCollectionTree(d,"root").collections.map(c=>c.id)).toEqual(["other"]);
});
