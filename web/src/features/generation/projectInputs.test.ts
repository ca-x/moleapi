import {base64} from "@scure/base";
import {afterEach,expect,it,vi} from "vitest";
import {pickProjectSource,readBrowserProject} from "./projectInputs";
const local=vi.hoisted(()=>({native:false,open:vi.fn(),lstat:vi.fn(),readDir:vi.fn(),openFile:vi.fn()}));
vi.mock("../../shared/api",()=>({get native(){return local.native;}}));
vi.mock("@tauri-apps/plugin-dialog",()=>({open:local.open}));
vi.mock("@tauri-apps/plugin-fs",()=>({lstat:local.lstat,readDir:local.readDir,open:local.openFile}));
vi.mock("@tauri-apps/api/path",()=>({join:(...parts:string[])=>Promise.resolve(parts.join("/"))}));
const browserFile=(path:string,bytes:Uint8Array,size=bytes.length)=>({webkitRelativePath:path,size,arrayBuffer:()=>Promise.resolve(bytes.buffer)} as File);
afterEach(()=>{vi.clearAllMocks();local.native=false;});
it("browser directories preserve binary bytes, omit dependency directories and remove only the selected root",async()=>{
 const result=await readBrowserProject([browserFile("project/src/raw.bin",new Uint8Array([255,0,128])),browserFile("project/target/huge",new Uint8Array(),100*1024*1024)],()=>true);
 expect(result).toEqual({kind:"files",files:[{path:"src/raw.bin",encoding:"base64",content:base64.encode(new Uint8Array([255,0,128])),executable:false}]});
});
it("browser inputs reject traversal, inconsistent roots and oversized content before reading",async()=>{
 for(const values of [[browserFile("project/../escape",new Uint8Array())],[browserFile("project/a",new Uint8Array()),browserFile("other/b",new Uint8Array())],[browserFile("project/large",new Uint8Array(),5*1024*1024)]]){
  await expect(readBrowserProject(values,()=>true)).rejects.toThrow();
 }
 expect(await readBrowserProject([browserFile("project/a",new Uint8Array())],()=>false)).toBeNull();
});
it("native directory traversal grants only a dialog-selected root and preserves executable flags",async()=>{
 local.native=true;local.open.mockResolvedValue("/selected");
 local.readDir.mockResolvedValue([{name:"run",isDirectory:false,isFile:true,isSymlink:false},{name:"target",isDirectory:true,isFile:false,isSymlink:false}]);
 local.lstat.mockImplementation((path:string)=>Promise.resolve(path==="/selected"?{isDirectory:true,isFile:false,isSymlink:false}:{isDirectory:false,isFile:true,isSymlink:false,size:2}));
 let read=false;const close=vi.fn();
 local.openFile.mockResolvedValue({stat:()=>Promise.resolve({isFile:true,size:2,mode:0o100755}),read:(buffer:Uint8Array)=>{if(read)return Promise.resolve(null);read=true;buffer.set([255,0]);return Promise.resolve(2);},close});
 expect(await pickProjectSource("directory",()=>true)).toEqual({kind:"files",files:[{path:"run",encoding:"base64",content:base64.encode(new Uint8Array([255,0])),executable:true}]});
 expect(local.open).toHaveBeenCalledWith(expect.objectContaining({directory:true,recursive:true,multiple:false}));
 expect(local.readDir).toHaveBeenCalledTimes(1);expect(close).toHaveBeenCalledTimes(1);
});
it("native selected links are rejected without reading their targets",async()=>{
 local.native=true;local.open.mockResolvedValue("/selected.zip");local.lstat.mockResolvedValue({isFile:true,isSymlink:true,size:0});
 await expect(pickProjectSource("zip",()=>true)).rejects.toThrow();expect(local.openFile).not.toHaveBeenCalled();
});
it("native ZIP reads stop after owner invalidation and close the acquired handle",async()=>{
 local.native=true;local.open.mockResolvedValue("/selected.zip");local.lstat.mockResolvedValue({isFile:true,isSymlink:false,size:2});
 let valid=true;const close=vi.fn(),read=vi.fn(async()=>{valid=false;return 1;});
 local.openFile.mockResolvedValue({stat:()=>Promise.resolve({isFile:true,size:2,mode:null}),read,close});
 expect(await pickProjectSource("zip",()=>valid)).toBeNull();expect(read).toHaveBeenCalledTimes(1);expect(close).toHaveBeenCalledTimes(1);
});

it("directory pickers retain known executable flags when the browser cannot expose file modes",async()=>{
 const result=await readBrowserProject([browserFile("project/run",new Uint8Array([1]))],()=>true,new Map([["run",true]]));
 expect(result).toMatchObject({kind:"files",files:[{path:"run",executable:true}]});
});
