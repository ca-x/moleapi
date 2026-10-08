// @vitest-environment jsdom
import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,expect,it,vi} from "vitest";
import {api} from "../../shared/api";
import {setLanguage} from "../../shared/i18n";
import CookieManager from "./CookieManager";
vi.mock("../../shared/api",()=>({api:vi.fn()}));
afterEach(()=>{cleanup();vi.clearAllMocks();vi.unstubAllGlobals();});
const entry={domain:"example.com",path:"/",name:"sid",value:"[REDACTED]",host_only:true,secure:true,http_only:true,same_site:"Lax",expires:"SessionEnd"};
async function open(){await setLanguage("en");vi.stubGlobal("ResizeObserver",class{observe(){}unobserve(){}disconnect(){}});render(<Theme><CookieManager workspace="workspace" environment="dev" url="https://example.com/"/></Theme>);fireEvent.click(screen.getByRole("button",{name:"Manage cookies"}));}
it("masks values and uses the selected environment for reveal and clear",async()=>{
 vi.mocked(api).mockResolvedValue({enabled:true,cookies:[entry]});await open();
 const input=await screen.findByLabelText("Cookie value: sid");expect((input as HTMLInputElement).type).toBe("password");
 expect(api).toHaveBeenCalledWith("/api/workspaces/workspace/cookies?environment_id=dev","GET",undefined);
 vi.mocked(api).mockResolvedValue({enabled:true,cookies:[{...entry,value:"private"}]});fireEvent.click(screen.getByRole("button",{name:"Reveal cookie values"}));
 await waitFor(()=>expect((screen.getByLabelText("Cookie value: sid") as HTMLInputElement).type).toBe("text"));
 expect(api).toHaveBeenLastCalledWith("/api/workspaces/workspace/cookies?environment_id=dev&reveal=true","GET",undefined);
 vi.mocked(api).mockResolvedValue({enabled:true,cookies:[]});fireEvent.click(screen.getByRole("button",{name:"Clear cookies"}));await screen.findByText("No cookies in the current environment.");
 expect(api).toHaveBeenLastCalledWith("/api/workspaces/workspace/cookies?environment_id=dev","PATCH",{enabled:true,clear:true});
});
it("ignores a response from a closed dialog when the dialog is reopened",async()=>{
 let finish!:(result:unknown)=>void;
 vi.mocked(api).mockReturnValueOnce(new Promise(resolve=>{finish=resolve;})).mockResolvedValue({enabled:false,cookies:[]});await open();
 fireEvent.click(screen.getByRole("button",{name:"Done"}));fireEvent.click(screen.getByRole("button",{name:"Manage cookies"}));await screen.findByText("No cookies in the current environment.");
 await act(async()=>{finish({enabled:true,cookies:[entry]});});expect(screen.queryByLabelText("Cookie value: sid")).toBeNull();
});
