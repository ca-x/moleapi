export type PublishedSearchDocument={title:string;folder:string|null;source:string};
import {unicodeCaseFold} from "./unicodeCaseFold";

export function indexPublishedDocuments(documents:ReadonlyArray<PublishedSearchDocument>):string[] {
  return documents.map(document=>unicodeCaseFold(`${document.title}\n${document.folder??""}\n${document.source}`));
}

export function searchPublishedDocuments(index:ReadonlyArray<string>,query:string):number[] {
  const bounded=Array.from(query).slice(0,128).join("");
  const terms=unicodeCaseFold(bounded).trim().split(/\s+/u).filter(Boolean);
  if(!terms.length)return index.map((_,position)=>position);
  const matches:number[]=[];
  index.forEach((text,position)=>{
    if(terms.every(term=>text.includes(term)))matches.push(position);
  });
  return matches;
}
