import {base64} from "@scure/base";
import {native} from "../../shared/api";
import {t} from "../../shared/i18n";
import {LocalizedError} from "../../shared/i18n/errors";

export interface WorkingFile {path:string;encoding:"utf8"|"base64";content:string;executable:boolean}
export type ProjectImportSource = {kind:"zip";archive_base64:string}|{kind:"files";files:WorkingFile[]};
const FILE_LIMIT=4*1024*1024, TOTAL_LIMIT=8*1024*1024+512*1024, ARCHIVE_LIMIT=10*1024*1024;
const ignored=new Set([".git",".svn",".hg","node_modules","target",".venv","__pycache__"]);
const fail=()=>new LocalizedError("项目导入失败：最多 2048 个文件，每个 4 MiB，项目 8 MiB，ZIP 10 MiB；不允许链接或不安全路径。");

export function projectRelativePath(path:string):string {
 if(!path||path.length>1024||/[\\\0:]/.test(path)||path.split("/").some(segment=>!segment||segment==="."||segment===".."))throw fail();
 return path;
}
export function includedProjectPath(path:string):boolean {
 return !projectRelativePath(path).split("/").slice(0,-1).some(segment=>ignored.has(segment));
}
async function nativeBytes(path:string,maximum:number,current:()=>boolean):Promise<{bytes:Uint8Array;executable:boolean|null}|null> {
 const {lstat,open}=await import("@tauri-apps/plugin-fs");
 if(!current())return null;
 const info=await lstat(path);
 if(!current())return null;
 if(!info.isFile||info.isSymlink||info.size>maximum)throw fail();
 const file=await open(path,{read:true});
 try {
  const metadata=await file.stat();
  if(!metadata.isFile||metadata.size>maximum)throw fail();
  const bytes=new Uint8Array(Math.min(maximum,Math.max(info.size,metadata.size))+1);
  let size=0;
  while(size<bytes.length){
   if(!current())return null;
   const count=await file.read(bytes.subarray(size));
   if(!count)break;size+=count;
  }
  if(size>maximum)throw fail();
  // A file growing beyond its advertised size must also stay within the budget.
  if(size===bytes.length)throw fail();
  return {bytes:bytes.subarray(0,size),executable:metadata.mode===null?null:(metadata.mode&0o111)!==0};
 } finally {await file.close();}
}

async function browserSelection(directory:boolean):Promise<File[]|null> {
 return new Promise(resolve=>{
  const input=document.createElement("input");input.type="file";input.hidden=true;
  input.setAttribute("aria-label",directory?t("选择当前项目目录"):t("选择项目 ZIP"));
  if(directory){input.webkitdirectory=true;input.multiple=true;}else input.accept=".zip";
  document.body.append(input);
  input.addEventListener("cancel",()=>{input.remove();resolve(null);},{once:true});
  input.onchange=()=>{const files=Array.from(input.files??[]);input.remove();resolve(files.length?files:null);};
  input.click();
 });
}
export async function readBrowserProject(files:File[],current:()=>boolean,knownModes:Map<string,boolean>=new Map()):Promise<ProjectImportSource|null> {
 const roots=new Set(files.map(file=>file.webkitRelativePath.split("/")[0]));
 if(roots.size>1)throw fail();
 const picked=files.map(file=>{
  const path=file.webkitRelativePath;
  if(!path||!path.includes("/"))throw fail();
  projectRelativePath(path);
  return {file,path:path.slice(path.indexOf("/")+1)};
 }).filter(({path})=>includedProjectPath(path));
 if(picked.length>2048||picked.some(({file})=>file.size>FILE_LIMIT)||picked.reduce((sum,{file})=>sum+file.size,0)>TOTAL_LIMIT)throw fail();
 const result:WorkingFile[]=[];
 let total=0;
 for(const {file,path} of picked){
  if(!current())return null;
  const bytes=new Uint8Array(await file.arrayBuffer());total+=bytes.length;
  if(bytes.length>FILE_LIMIT||total>TOTAL_LIMIT)throw fail();
  result.push({path,encoding:"base64",content:base64.encode(bytes),executable:knownModes.get(path)??false});
 }
 return current()?{kind:"files",files:result}:null;
}
export async function pickProjectSource(kind:"zip"|"directory",current:()=>boolean,knownModes:Map<string,boolean>=new Map()):Promise<ProjectImportSource|null> {
 if(!current())return null;
 if(!native){
  const selected=await browserSelection(kind==="directory");
  if(!selected||!current())return null;
  if(kind==="directory")return readBrowserProject(selected,current,knownModes);
  const file=selected[0];if(file.size>ARCHIVE_LIMIT)throw fail();
  const bytes=new Uint8Array(await file.arrayBuffer());
  if(bytes.length>ARCHIVE_LIMIT)throw fail();
  return current()?{kind:"zip",archive_base64:base64.encode(bytes)}:null;
 }
 const [{open},{join}]=await Promise.all([import("@tauri-apps/plugin-dialog"),import("@tauri-apps/api/path")]);
 if(!current())return null;
 const path=await open({directory:kind==="directory",recursive:kind==="directory",multiple:false,filters:kind==="zip"?[{name:t("项目 ZIP"),extensions:["zip"]}]:undefined});
 if(typeof path!=="string"||!current())return null;
 if(kind==="zip"){
  const file=await nativeBytes(path,ARCHIVE_LIMIT,current);
  return file&&current()?{kind:"zip",archive_base64:base64.encode(file.bytes)}:null;
 }
 const {readDir,lstat}=await import("@tauri-apps/plugin-fs");
 const root=await lstat(path);if(!root.isDirectory||root.isSymlink)throw fail();
 const pending=[{absolute:path,relative:""}],files:WorkingFile[]=[];
 let nodes=0,total=0;
 while(pending.length){
  if(!current())return null;
  const directory=pending.pop()!;
  const entries=await readDir(directory.absolute);
  for(const entry of entries){
   if(!current())return null;
   if(++nodes>4096)throw fail();
   projectRelativePath(entry.name);
   if(entry.name.includes("/"))throw fail();
   if(entry.isDirectory&&ignored.has(entry.name))continue;
   if(entry.isSymlink||(!entry.isFile&&!entry.isDirectory))throw fail();
   const relative=projectRelativePath(directory.relative?`${directory.relative}/${entry.name}`:entry.name);
   const absolute=await join(directory.absolute,entry.name);
   const metadata=await lstat(absolute);
   if(metadata.isSymlink||metadata.isDirectory!==entry.isDirectory||metadata.isFile!==entry.isFile)throw fail();
   if(entry.isDirectory){pending.push({absolute,relative});continue;}
   if(files.length>=2048)throw fail();
   const file=await nativeBytes(absolute,Math.min(FILE_LIMIT,TOTAL_LIMIT-total),current);
   if(!file)return null;
   total+=file.bytes.length;
   files.push({path:relative,encoding:"base64",content:base64.encode(file.bytes),executable:file.executable??knownModes.get(relative)??false});
  }
 }
 return current()?{kind:"files",files}:null;
}
