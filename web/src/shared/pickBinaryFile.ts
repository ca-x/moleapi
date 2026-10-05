import { base64 } from "@scure/base";
import {t} from "./i18n";
import { native } from "./api";
import { LocalizedError } from "./i18n/errors";
const MAX=5*1024*1024;
export async function pickBinaryFile(options:{extensions?:string[];accept?:string}={}):Promise<{name:string;base64:string;mime:string}|null> {
  if(native){
    const [{open},{open:openFile,stat},{basename}]=await Promise.all([import("@tauri-apps/plugin-dialog"),import("@tauri-apps/plugin-fs"),import("@tauri-apps/api/path")]);
    const path=await open({multiple:false,filters:options.extensions?[{name:t("文件"),extensions:options.extensions}]:undefined});
    if(typeof path!=="string")return null;
    const metadata=await stat(path);if(metadata.size>MAX)throw new LocalizedError("上传文件超过 5 MiB。");
    const file=await openFile(path,{read:true});let count=0;const buffer=new Uint8Array(MAX+1);
    try {while(count<buffer.length){const read=await file.read(buffer.subarray(count));if(read===null||read===0)break;count+=read;}}finally{await file.close();}
    if(count>MAX)throw new LocalizedError("上传文件超过 5 MiB。");
    return {name:await basename(path),base64:base64.encode(buffer.subarray(0,count)),mime:""};
  }
  return new Promise((resolve,reject)=>{
    const input=document.createElement("input");input.type="file";input.accept=options.accept??"";input.hidden=true;document.body.append(input);
    input.addEventListener("cancel",()=>{input.remove();resolve(null);});
    input.onchange=()=>{const file=input.files?.[0];input.remove();if(!file){resolve(null);return;}if(file.size>MAX){reject(new LocalizedError("上传文件超过 5 MiB。"));return;}
      file.arrayBuffer().then(buffer=>resolve({name:file.name,base64:base64.encode(new Uint8Array(buffer)),mime:file.type}),reject);};input.click();
  });
}
