import postman from 'postman-code-generators';
import {Request} from 'postman-collection/lib/collection/request.js';
const languages={curl:'shell',javascript:'js',nodejs:'node','objective-c':'objc'};
function clientId(language,variant){
 const value=variant.toLowerCase();
 if(language==='python'&&value==='http.client')return 'python3';
 if(language==='ruby'&&value==='net::http')return 'native';
 if(language==='http')return 'http1.1';
 if(value==='urlsession')return 'nsurlsession';
 return value.replaceAll(' ','_').replace('http_request2','httprequest2');
}
const implementations=new Map();
export function mergePostmanCatalog(scalar){
 const result=scalar.map(target=>({...target,clients:target.clients.map(client=>({...client}))}));
 for(const language of postman.getLanguageList()){
  const target=languages[language.key]??language.key;
  let row=result.find(row=>row.target===target);
  if(!row){row={target,title:language.label,clients:[]};result.push(row);}
  for(const variant of language.variants){
   const client=clientId(language.key,variant.key);
   if(row.clients.some(entry=>entry.client===client))continue;
   row.clients.push({client,title:variant.key});
   implementations.set(`${target}/${client}`,{language:language.key,variant:variant.key});
  }
 }
 const shell=result.find(row=>row.target==='shell');
 shell.clients.push({client:'curl_windows',title:'cURL (Windows cmd.exe)'});
 implementations.set('shell/curl_windows',{language:'curl',variant:'cURL',options:{quoteType:'double',lineContinuationCharacter:'^',multiLine:false}});
 const js=result.find(row=>row.target==='js');
 for(const client of ['native','request','unirest']){
  js.clients.push({client,title:`${client==='native'?'Native':client==='request'?'Request':'Unirest'} (Node.js)`});
  implementations.set(`js/${client}`,implementations.get(`node/${client}`));
 }
 return result;
}
export function postmanSnippet(target,client,har){
 const implementation=implementations.get(`${target}/${client}`);
 if(!implementation)return null;
 const body=har.postData;
 const request=new Request({method:har.method,url:har.url,header:(har.headers??[]).map(header=>({key:header.name,value:header.value})),body:!body?undefined:body.mimeType==='application/x-www-form-urlencoded'?{mode:'urlencoded',urlencoded:(body.params??[]).map(parameter=>({key:parameter.name,value:parameter.value??''}))}:{mode:'raw',raw:body.text??'',options:{raw:{language:body.mimeType==='application/json'?'json':'text'}}}});
 let complete=false,error=null,code=null;
 postman.convert(implementation.language,implementation.variant,request,{...implementation.options,trimRequestBody:false,addCacheHeader:false},(failure,result)=>{complete=true;error=failure;code=result;});
 if(!complete||error||typeof code!=='string'||!code)throw new Error('Postman request generator failed');
 return code;
}
