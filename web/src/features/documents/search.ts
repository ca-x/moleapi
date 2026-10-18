import type {WorkspaceData} from "../../shared/types";

export type SearchResult={kind:"document"|"request";id:string;title:string;context:string;excerpt:string};
export type SearchResponse={results:SearchResult[];total:number;truncated:boolean};
const RESULT_LIMIT=100;
const QUERY_LIMIT=128;
const EXCERPT_LIMIT=160;

function excerpt(source:string,term:string):string {
  const text=source.replace(/\s+/g," ").trim();
  const chars=Array.from(text);
  if(chars.length<=EXCERPT_LIMIT)return text;
  const match=text.toLowerCase().indexOf(term);
  const position=match<0?0:Array.from(text.slice(0,match)).length;
  const start=Math.max(0,Math.min(position-40,chars.length-(EXCERPT_LIMIT-2)));
  const length=EXCERPT_LIMIT-(start>0?1:0)-(start+EXCERPT_LIMIT-2<chars.length?1:0);
  return `${start>0?"…":""}${chars.slice(start,start+length).join("")}${start+length<chars.length?"…":""}`;
}

export function searchWorkspace(data:WorkspaceData,raw:string):SearchResponse {
  const empty:SearchResponse={results:[],total:0,truncated:false};
  if(Array.from(raw).length>QUERY_LIMIT)return empty;
  const terms=raw.trim().toLowerCase().split(/\s+/).filter(Boolean);
  if(!terms.length)return empty;
  const found:Array<SearchResult&{rank:number;order:number}>=[];
  let order=0;
  const add=(kind:SearchResult["kind"],id:string,title:string,context:string,fields:string[])=>{
    const indexed=[title,...fields].map(value=>value.toLowerCase());
    const ordinal=order++;
    if(!terms.every(term=>indexed.some(value=>value.includes(term))))return;
    const rank=terms.every(term=>indexed[0].includes(term))?0:1;
    const source=fields.find(value=>terms.some(term=>value.toLowerCase().includes(term)))??fields[0]??"";
    found.push({kind,id,title,context,excerpt:excerpt(source,terms[0]),rank,order:ordinal});
  };
  for(const document of data.documents??[]) {
    const context=data.collections.find(collection=>collection.id===document.collection_id)?.name??"";
    add("document",document.id,document.name,context,[document.source]);
  }
  for(const collection of data.collections)for(const request of collection.requests) {
    const fields=[request.method,request.url,request.description,
      ...(["json","text","form"].includes(request.body_kind)?[request.body]:[]),
      ...request.query.filter(row=>row.enabled).map(row=>row.key),
      ...request.headers.filter(row=>row.enabled).map(row=>row.key),
      ...request.examples.flatMap(example=>[example.name,example.body])];
    add("request",request.id,request.name,collection.name,fields);
  }
  found.sort((a,b)=>a.rank-b.rank||a.order-b.order);
  return {total:found.length,truncated:found.length>RESULT_LIMIT,results:found.slice(0,RESULT_LIMIT).map(({kind,id,title,context,excerpt})=>({kind,id,title,context,excerpt}))};
}
