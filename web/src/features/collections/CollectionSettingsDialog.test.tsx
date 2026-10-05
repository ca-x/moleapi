// @vitest-environment jsdom
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import type { WorkspaceData } from "../../shared/types";
import {initialData} from "../../shared/model";
import {setLanguage} from "../../shared/i18n";
import {useWorkbench} from "../workbench/context";
import CollectionSettingsDialog from "./CollectionSettingsDialog";
vi.mock("../workbench/context",()=>({useWorkbench:vi.fn()}));
let state:ReturnType<typeof useWorkbench>;
beforeEach(()=>{
 vi.stubGlobal("ResizeObserver",class {observe(){} unobserve(){} disconnect(){}});
 setLanguage("en");const data=initialData();data.collections[0].id="root";
 state={accountId:"owner",accountRef:{current:"owner"},draft:{id:"workspace",data},stateRef:{current:{draft:{id:"workspace",data}}},dark:false,localVariables:{reconcile:vi.fn(()=>true)},updateData:vi.fn((update:(data:WorkspaceData)=>WorkspaceData)=>{state.draft!.data=update(state.draft!.data);})} as unknown as ReturnType<typeof useWorkbench>;
 vi.mocked(useWorkbench).mockImplementation(()=>state);
});
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
it("creates a real folder under its parent and keeps changes local until Apply",()=>{
 const close=vi.fn();render(<Theme><CollectionSettingsDialog target={{kind:"create",parent_id:"root"}} close={close}/></Theme>);
 fireEvent.change(screen.getByRole("textbox",{name:"Name"}),{target:{value:"Folder"}});
 expect(state.draft!.data.collections).toHaveLength(1);
 fireEvent.click(screen.getByRole("button",{name:"Apply"}));
 expect(state.draft!.data.collections).toHaveLength(2);
 expect(state.draft!.data.collections[1]).toMatchObject({name:"Folder",parent_id:"root",requests:[]});expect(close).toHaveBeenCalled();
});
it("preserves concurrent request edits while updating parent metadata",()=>{
 render(<Theme><CollectionSettingsDialog target={{kind:"edit",id:"root"}} close={vi.fn()}/></Theme>);
 fireEvent.change(screen.getByRole("textbox",{name:"Name"}),{target:{value:"Renamed"}});
 state.draft!.data.collections[0].requests[0].name="New request edit";
 fireEvent.click(screen.getByRole("button",{name:"Apply"}));
 expect(state.draft!.data.collections[0].name).toBe("Renamed");expect(state.draft!.data.collections[0].requests[0].name).toBe("New request edit");
});
it("rejects metadata changes and stale workspace owners without overwriting source",()=>{
 const close=vi.fn();render(<Theme><CollectionSettingsDialog target={{kind:"edit",id:"root"}} close={close}/></Theme>);
 state.draft!.data.collections[0].name="Remote edit";
 fireEvent.click(screen.getByRole("button",{name:"Apply"}));
 expect(screen.getByRole("alert").textContent).toContain("changed while editing");expect(state.draft!.data.collections[0].name).toBe("Remote edit");expect(close).not.toHaveBeenCalled();
 state.accountRef.current="other";
 fireEvent.click(screen.getByRole("button",{name:"Apply"}));expect(close).toHaveBeenCalled();expect(state.draft!.data.collections[0].name).toBe("Remote edit");
});

it("reconciles variable renames only on successful Apply and retains pending edits on storage failure",()=>{
 state.draft!.data.collections[0].variables=[{id:"credential",key:"old-name",value:"shared",enabled:true}];
 render(<Theme><CollectionSettingsDialog target={{kind:"edit",id:"root"}} close={vi.fn()}/></Theme>);
 fireEvent.mouseDown(screen.getByRole("tab",{name:/Variables/}),{button:0,ctrlKey:false});
 const name=screen.getByDisplayValue("old-name");fireEvent.change(name,{target:{value:"renamed"}});
 expect(state.localVariables.reconcile).not.toHaveBeenCalled();
 vi.mocked(state.localVariables.reconcile).mockReturnValueOnce(false);
 fireEvent.click(screen.getByRole("button",{name:"Apply"}));
 expect(state.draft!.data.collections[0].variables![0].key).toBe("old-name");
 expect(screen.getByRole("alert").textContent).toContain("not completed");
 fireEvent.click(screen.getByRole("button",{name:"Apply"}));
 expect(state.localVariables.reconcile).toHaveBeenLastCalledWith("collection","root",expect.arrayContaining([expect.objectContaining({key:"old-name"})]),expect.arrayContaining([expect.objectContaining({key:"renamed"})]));
 expect(state.draft!.data.collections[0].variables![0].key).toBe("renamed");
});
