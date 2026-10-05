// @vitest-environment jsdom
import {useState} from "react";
import {act,cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import RequestBodyEditor from "./RequestBodyEditor";
import {emptyFile} from "./model";
import {pickBinaryFile} from "../../shared/pickBinaryFile";
vi.mock("../../shared/pickBinaryFile",()=>({pickBinaryFile:vi.fn()}));
vi.mock("../../shared/ui",()=>({Editor:()=>null,Choice:()=>null,Field:({label,children}:{label:string;children:React.ReactNode})=><label>{label}{children}</label>}));
afterEach(()=>{cleanup();vi.clearAllMocks();});
it("a late browser MIME suggestion cannot overwrite explicit MIME edits",async()=>{
 let finish!:(file:{name:string;base64:string;mime:string})=>void;
 vi.mocked(pickBinaryFile).mockReturnValue(new Promise(done=>{finish=done;}));
 function Harness(){const[value,setValue]=useState(JSON.stringify(emptyFile()));return <RequestBodyEditor kind="binary" value={value} change={setValue} busy={false} dark={false}/>;}
 render(<Harness/>);fireEvent.click(screen.getByRole("button",{name:"选择上传文件"}));fireEvent.change(screen.getByRole("textbox",{name:"文件 MIME 类型"}),{target:{value:"application/custom"}});
 await act(async()=>{finish({name:"file.bin",base64:"AA==",mime:"image/png"});});
 expect((screen.getByRole("textbox",{name:"文件 MIME 类型"}) as HTMLInputElement).value).toBe("application/custom");expect(screen.getByText("file.bin")).toBeTruthy();
});
it("even a completed send invalidates a deferred picker snapshot",async()=>{
 let finish!:(file:{name:string;base64:string;mime:string})=>void;
 vi.mocked(pickBinaryFile).mockReturnValue(new Promise(done=>{finish=done;}));const change=vi.fn();const props={kind:"binary",value:JSON.stringify(emptyFile()),change,dark:false};
 const view=render(<RequestBodyEditor {...props} busy={false}/>);fireEvent.click(screen.getByRole("button",{name:"选择上传文件"}));view.rerender(<RequestBodyEditor {...props} busy/>);view.rerender(<RequestBodyEditor {...props} busy={false}/>);
 await act(async()=>{finish({name:"late.bin",base64:"AA==",mime:""});});expect(change).not.toHaveBeenCalled();
});
