import type {MarkdownDocument} from "../../shared/types";

export function validateMarkdownDocuments(documents:MarkdownDocument[]|undefined) {
  const items=documents??[];
  const encoder=new TextEncoder();
  const invalidTitleDocument=items.find(document=>document.name.trim()!==document.name||!document.name.trim()||[...document.name].length>128||/\p{Cc}/u.test(document.name));
  let invalidSourceDocument:MarkdownDocument|undefined;
  let totalSourceBytes=0;
  for(const document of items) {
    const bytes=encoder.encode(document.source).length;
    if(bytes>256*1024&&!invalidSourceDocument)invalidSourceDocument=document;
    totalSourceBytes+=bytes;
  }
  return {invalidTitleDocument,invalidSourceDocument,invalidTotalSource:totalSourceBytes>2*1024*1024};
}
