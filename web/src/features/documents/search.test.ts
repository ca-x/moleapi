import {expect,it} from "vitest";
import {newRequest} from "../../shared/model";
import type {WorkspaceData} from "../../shared/types";
import {searchWorkspace} from "./search";

function fixture():WorkspaceData {
  const request=newRequest("Guide API","https://example.test/animals");
  request.id="r1";
  request.method="POST";
  request.description="Create an animal";
  request.auth={kind:"bearer",token:"credential-never-index",username:"",password:""};
  request.query=[{id:"q1",key:"species",value:"secret-query-value",enabled:true},{id:"q2",key:"hidden-key",value:"",enabled:false}];
  request.headers=[{id:"h1",key:"X-Trace",value:"secret-header-value",enabled:true}];
  request.body_kind="json";
  request.body='{"kind":"cat"}';
  request.examples=[{id:"e1",name:"Created",status:201,headers:[],body:'{"pet":"Ada"}'}];
  const other=newRequest("Unrelated","https://example.test/other");
  other.id="r2";
  return {schema_version:1,collections:[{id:"c",name:"Pets",description:"",requests:[request,other]}],documents:[{id:"d1",name:"Guide notes",collection_id:"c",source:"Private Unicode 猫 care [x]?",secret:true},{id:"d2",name:"Other",collection_id:null,source:"Guide in the body",secret:false}],environments:[],active_environment_id:null};
}

it("ranks title matches before source matches while preserving original order",()=>{
  const data=fixture();
  const original=JSON.stringify(data);
  const result=searchWorkspace(data,"gUiDe");
  expect(result.results.map(item=>[item.kind,item.id])).toEqual([["document","d1"],["request","r1"],["document","d2"]]);
  expect(result.total).toBe(3);
  expect(JSON.stringify(data)).toBe(original);
});

it("matches literal Unicode and AND terms across documented fields, excluding credentials and disabled rows",()=>{
  const data=fixture();
  expect(searchWorkspace(data,"猫 care").results.map(item=>item.id)).toEqual(["d1"]);
  expect(searchWorkspace(data,"[x]?").results.map(item=>item.id)).toEqual(["d1"]);
  expect(searchWorkspace(data,"post animals").results.map(item=>item.id)).toEqual(["r1"]);
  expect(searchWorkspace(data,"species").results.map(item=>item.id)).toEqual(["r1"]);
  expect(searchWorkspace(data,"Ada").results.map(item=>item.id)).toEqual(["r1"]);
  for(const secret of ["credential-never-index","secret-query-value","secret-header-value","hidden-key"]) {
    expect(searchWorkspace(data,secret).results).toEqual([]);
  }
  expect(searchWorkspace(data,"Pets").results).toEqual([]);
});

it("bounds snippets, query length and rendered results without losing total count",()=>{
  const data=fixture();
  data.documents=Array.from({length:101},(_,index)=>({id:`d-${index}`,name:`Note ${index}`,source:`${"x".repeat(200)} needle ${"y".repeat(200)}`,collection_id:null,secret:true}));
  const result=searchWorkspace(data,"needle");
  expect(result.total).toBe(101);
  expect(result.results).toHaveLength(100);
  expect(result.truncated).toBe(true);
  expect(result.results[0].excerpt).toContain("needle");
  expect([...result.results[0].excerpt].length).toBeLessThanOrEqual(160);
  expect(searchWorkspace(data," ").results).toEqual([]);
  expect(searchWorkspace(data,"x".repeat(129)).results).toEqual([]);
});
