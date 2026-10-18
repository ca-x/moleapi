const REQUEST_LINK_PREFIX="moleapi://request/";

export function formatRequestLink(id:string):string {
  if(!id)throw new Error("Request ID is required");
  return REQUEST_LINK_PREFIX+encodeURIComponent(id);
}

export function parseRequestLink(raw:string):string|null {
  if(!raw.startsWith(REQUEST_LINK_PREFIX))return null;
  const encoded=raw.slice(REQUEST_LINK_PREFIX.length);
  if(!encoded||/[/?#]/.test(encoded))return null;
  try {
    const id=decodeURIComponent(encoded);
    return id&&encodeURIComponent(id)===encoded?id:null;
  } catch {return null;}
}
