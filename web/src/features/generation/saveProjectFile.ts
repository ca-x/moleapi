import {base64} from "@scure/base";
import {native} from "../../shared/api";
import {t} from "../../shared/i18n";
export async function saveProjectFile(filename:string,bytes:Uint8Array,current:()=>boolean){
 if(!current())return;
 if(native){
   const [{save},{writeFile}]=await Promise.all([import("@tauri-apps/plugin-dialog"),import("@tauri-apps/plugin-fs")]);
   if(!current())return;
   const extension=filename.split(".").pop()??"txt";
   const path=await save({defaultPath:filename,filters:[{name:t("生成文件"),extensions:[extension]}]});
   if(path&&current())await writeFile(path,bytes);return;
 }
 const copy=new Uint8Array(bytes);const url=URL.createObjectURL(new Blob([copy.buffer]));const a=document.createElement("a");a.href=url;a.download=filename;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);
}
export const decodeProjectBytes=(encoding:string,content:string)=>encoding==="base64"?base64.decode(content):new TextEncoder().encode(content);
