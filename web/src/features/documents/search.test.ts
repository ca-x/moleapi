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
  data.collections[0].requests[0].url="https://user:private-url-password@example.test/animals?token=inline-secret&visible=yes#fragment-secret";
  expect(searchWorkspace(data,"猫 care").results.map(item=>item.id)).toEqual(["d1"]);
  expect(searchWorkspace(data,"[x]?").results.map(item=>item.id)).toEqual(["d1"]);
  expect(searchWorkspace(data,"post animals").results.map(item=>item.id)).toEqual(["r1"]);
  expect(searchWorkspace(data,"species").results.map(item=>item.id)).toEqual(["r1"]);
  expect(searchWorkspace(data,"Ada").results.map(item=>item.id)).toEqual(["r1"]);
  for(const secret of ["credential-never-index","secret-query-value","secret-header-value","hidden-key","private-url-password","inline-secret","fragment-secret"]) {
    expect(searchWorkspace(data,secret).results).toEqual([]);
  }
  const urlMatch=searchWorkspace(data,"example.test").results[0];
  expect(urlMatch.id).toBe("r1");
  expect(urlMatch.excerpt).not.toContain("private-url-password");
  expect(urlMatch.excerpt).not.toContain("inline-secret");
  data.collections[0].requests[0].url="{{base_url}}/animals?api_key=template-secret#frag-secret";
  expect(searchWorkspace(data,"animals").results.map(item=>item.id)).toEqual(["r1"]);
  expect(searchWorkspace(data,"template-secret").results).toEqual([]);
  expect(searchWorkspace(data,"frag-secret").results).toEqual([]);
  for(const url of ["{{scheme}}://user:templated-password@example.test/animals","//user:templated-password@example.test/animals"]) {
    data.collections[0].requests[0].url=url;
    expect(searchWorkspace(data,"templated-password").results).toEqual([]);
    const matching=searchWorkspace(data,"example.test").results;
    expect(matching.map(item=>item.id)).toContain("r1");
    expect(matching.find(item=>item.id==="r1")?.excerpt).not.toContain("templated-password");
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
  data.documents=[{id:"unicode",name:"Unicode",source:`${"İ".repeat(220)} needle ${"y".repeat(220)}`,collection_id:null,secret:true}];
  const unicode=searchWorkspace(data,"needle").results[0];
  expect(unicode.excerpt).toContain("needle");
  expect([...unicode.excerpt].length).toBeLessThanOrEqual(160);
  data.documents=[{id:"greek",name:"Greek",source:`${"x".repeat(220)} ΟΣ ${"y".repeat(220)}`,collection_id:null,secret:true}];
  const greek=searchWorkspace(data,"ος").results[0];
  expect(greek.excerpt.toLowerCase()).toContain("ος");
});
