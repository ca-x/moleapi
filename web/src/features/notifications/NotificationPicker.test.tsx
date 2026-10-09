// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {QueryClient,QueryClientProvider} from "@tanstack/react-query";
import {Theme} from "@radix-ui/themes";
import {afterEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import NotificationPicker from "./NotificationPicker";
const api=vi.hoisted(()=>vi.fn());vi.mock("../../shared/api",()=>({api}));vi.mock("../workbench/context",()=>({useWorkbench:()=>({accountId:"owner",authenticated:true,draft:{id:"w"}})}));afterEach(cleanup);
it("selects an owned target without exposing its withheld endpoint",async()=>{
 await setLanguage("en");api.mockResolvedValue([{id:"target",settings:{name:"Failures",enabled:true}}]);const change=vi.fn();render(<QueryClientProvider client={new QueryClient({defaultOptions:{queries:{retry:false}}})}><Theme><NotificationPicker label="Targets" value={[]} onChange={change} disabled={false}/></Theme></QueryClientProvider>);await screen.findByText("Failures");fireEvent.click(screen.getByRole("checkbox"));await waitFor(()=>expect(change).toHaveBeenCalledWith(["target"]));expect(api).toHaveBeenCalledWith("/api/workspaces/w/notifications");
});
