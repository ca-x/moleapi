import { base64 } from "@scure/base";
import { native } from "../../shared/api";
import { LocalizedError } from "../../shared/i18n/errors";
const MAX=5*1024*1024;
export async function pickDataFile():Promise<{name:string;base64:string}|null> {
  if(native){
    const [{open},{open:openFile,stat},{basename}]=await Promise.all([import("@tauri-apps/plugin-dialog"),import("@tauri-apps/plugin-fs"),import("@tauri-apps/api/path")]);
    const path=await open({multiple:false,filters:[{name:"CSV / JSON / Parquet",extensions:["csv","json","jsonl","ndjson","parquet"]}]});
    if(typeof path!=="string")return null;
    const metadata=await stat(path);if(metadata.size>MAX)throw new LocalizedError("数据文件超过 5 MiB。");
    const file=await openFile(path,{read:true});let count=0;const buffer=new Uint8Array(MAX+1);
    try {while(count<buffer.length){const read=await file.read(buffer.subarray(count));if(read===null||read===0)break;count+=read;}}finally{await file.close();}
    if(count>MAX)throw new LocalizedError("数据文件超过 5 MiB。");
    return {name:await basename(path),base64:base64.encode(buffer.subarray(0,count))};
  }
  return new Promise((resolve,reject)=>{
    const input=document.createElement("input");input.type="file";input.accept=".csv,.json,.jsonl,.ndjson,.parquet";
    input.addEventListener("cancel",()=>resolve(null));
    input.onchange=()=>{const file=input.files?.[0];if(!file){resolve(null);return;}if(file.size>MAX){reject(new LocalizedError("数据文件超过 5 MiB。"));return;}
      file.arrayBuffer().then(buffer=>resolve({name:file.name,base64:base64.encode(new Uint8Array(buffer))}),reject);};input.click();
  });
}
