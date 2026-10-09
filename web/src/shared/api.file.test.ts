// @vitest-environment jsdom
import {afterEach,expect,it,vi} from "vitest";
const files=vi.hoisted(()=>({dialog:vi.fn(),stat:vi.fn(),open:vi.fn()}));
vi.mock("@tauri-apps/plugin-dialog",()=>({open:files.dialog}));
vi.mock("@tauri-apps/plugin-fs",()=>({stat:files.stat,open:files.open}));
afterEach(()=>{vi.clearAllMocks();vi.unstubAllGlobals();vi.resetModules();});
it("applies the browser's text import limit before a native snapshot or definition is read",async()=>{
 vi.stubGlobal("window",{__TAURI_INTERNALS__:{}});
 files.dialog.mockResolvedValue("/selected/snapshot.json");
 files.stat.mockResolvedValue({isFile:true,size:21*1024*1024});
 const {pickFile}=await import("./api");
 await expect(pickFile()).rejects.toThrow();
 expect(files.stat).toHaveBeenCalledWith("/selected/snapshot.json");
 expect(files.open).not.toHaveBeenCalled();
});
