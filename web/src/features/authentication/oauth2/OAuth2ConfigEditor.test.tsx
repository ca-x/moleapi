// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {useState} from "react";
import {afterEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../../shared/i18n";
import {oauth2Config} from "./types";
import OAuth2ConfigEditor from "./OAuth2ConfigEditor";
vi.mock("./TokenManager",()=>({default:()=>null}));
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
it("preserves scope separators while editing multiple scopes",()=>{
 vi.stubGlobal("ResizeObserver",class {observe(){} unobserve(){} disconnect(){}});setLanguage("en");function Host(){const[config,setConfig]=useState(oauth2Config());return <Theme><OAuth2ConfigEditor config={config} change={setConfig}/><output data-testid="scopes">{JSON.stringify(config.scopes)}</output></Theme>;}
 render(<Host/>);const input=screen.getByRole("textbox",{name:/^Scopes/});
 fireEvent.change(input,{target:{value:"read "}});expect((input as HTMLInputElement).value).toBe("read ");
 fireEvent.change(input,{target:{value:"read write"}});expect(screen.getByTestId("scopes").textContent).toBe('["read","write"]');
});
