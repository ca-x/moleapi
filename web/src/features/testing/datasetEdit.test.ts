import { expect, it } from "vitest";
import { datasetEdit, datasetFromEdit, validDatasetEdit } from "./datasetEdit";
it("retains omitted source and view SQL without inventing replacement data",()=>{
  const saved={id:"d",name:"Backup",table_name:"orders",description:"keep",secret:true,views:[{id:"view",name:"Restored"}]};
  expect(datasetFromEdit(datasetEdit(saved))).toMatchObject(saved);expect(datasetFromEdit(datasetEdit(saved)).source).toBeUndefined();
});
it("switches to a private query reference without serializing the prior literal source",()=>{
  const edit=datasetEdit({id:"d",name:"Rows",description:"",secret:false,source:{format:"json",source:'[{"private":"original"}]'}});edit.mode="query";edit.requestId="query";
  expect(validDatasetEdit(edit,[],[{value:"query"}])).toBe(true);const value=datasetFromEdit(edit);
  expect(value).toMatchObject({secret:true,query:{request_id:"query"}});expect(value.source).toBeUndefined();expect(JSON.stringify(value)).not.toContain("original");
  expect(validDatasetEdit(edit,[],[])).toBe(false);
});
it("matches byte and aggregate quotas while allowing an existing source to be replaced",()=>{
  const edit=datasetEdit();edit.name="Rows";edit.source="x";
  const datasets=Array.from({length:4},(_,index)=>({id:String(index),name:"Rows",description:"",secret:true,source:{format:"json" as const,source:"x".repeat(1024*1024)}}));
  expect(validDatasetEdit(edit,datasets,[])).toBe(false);edit.id="0";expect(validDatasetEdit(edit,datasets,[])).toBe(true);
  edit.name="数".repeat(86);expect(validDatasetEdit(edit,[],[])).toBe(false);edit.name="Rows";
  edit.views=Array.from({length:17},(_,index)=>({id:String(index),name:"View",sql:"x".repeat(65536)}));expect(validDatasetEdit(edit,[],[])).toBe(false);
});
