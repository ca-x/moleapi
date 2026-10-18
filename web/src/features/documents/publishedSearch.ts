export type PublishedSearchDocument={title:string;folder:string|null;source:string};

const fold=(value:string)=>value.normalize("NFKC").toLocaleLowerCase();

export function searchPublishedDocuments(documents:ReadonlyArray<PublishedSearchDocument>,query:string):number[] {
  const bounded=Array.from(query).slice(0,128).join("");
  const terms=fold(bounded).trim().split(/\s+/u).filter(Boolean);
  if(!terms.length)return documents.map((_,index)=>index);
  const matches:number[]=[];
  documents.forEach((document,index)=>{
    const text=fold(`${document.title}\n${document.folder??""}\n${document.source}`);
    if(terms.every(term=>text.includes(term)))matches.push(index);
  });
  return matches;
}
