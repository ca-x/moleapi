// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen,act} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import {pickBinaryFile} from "../../shared/pickBinaryFile";
import NetworkSettings,{defaultNetwork} from "./NetworkSettings";
vi.mock("../../shared/pickBinaryFile",()=>({pickBinaryFile:vi.fn()}));
vi.stubGlobal("ResizeObserver",class{observe(){}unobserve(){}disconnect(){}});
afterEach(()=>{cleanup();vi.clearAllMocks();});
it("preserves inactive identity fields when editing proxy and switches labels live",async()=>{
 await setLanguage("en");const change=vi.fn();const c=defaultNetwork();c.proxy.enabled=true;c.identity.key_pem="{{key}}";
 render(<Theme><NetworkSettings value={c} change={change}/></Theme>);
 fireEvent.change(screen.getByLabelText("Proxy password"),{target:{value:"{{proxy_password}}"}});
 expect(change.mock.calls[0][0].identity.key_pem).toBe("{{key}}");expect(change.mock.calls[0][0].proxy.password).toBe("{{proxy_password}}");
 await act(()=>setLanguage("zh-CN"));expect(screen.getByLabelText("代理密码")).toBeTruthy();
});
it("discards late certificate selection after config changes or unmount",async()=>{
 await setLanguage("en");let finish!:(value:{name:string;base64:string;mime:string})=>void;
 vi.mocked(pickBinaryFile).mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));
 const c=defaultNetwork();const change=vi.fn();const view=render(<Theme><NetworkSettings value={c} change={change}/></Theme>);
 fireEvent.click(screen.getByRole("button",{name:"Select certificate file"}));
 view.rerender(<Theme><NetworkSettings value={{...c,ca_pem:"manual"}} change={change}/></Theme>);
 await act(async()=>finish({name:"test.pem",base64:"dGVzdA==",mime:""}));expect(change).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole("button",{name:"Select certificate file"}));view.unmount();
 await act(async()=>finish({name:"test.pem",base64:"dGVzdA==",mime:""}));expect(change).not.toHaveBeenCalled();
});
it("creates HTTP2 settings when editing a gRPC request with no existing network config",async()=>{
 await setLanguage("en");const change=vi.fn();render(<Theme><NetworkSettings grpc change={change}/></Theme>);
 fireEvent.change(screen.getByLabelText("Connect timeout (ms)"),{target:{value:"2000"}});
 expect(change.mock.calls[0][0].http_mode).toBe("auto");expect(change.mock.calls[0][0].connect_timeout_ms).toBe(2000);
 expect(screen.getByText("gRPC uses HTTP/2; TLS, DNS and proxy settings also apply.")).toBeTruthy();
});
