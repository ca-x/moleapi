// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {useState} from "react";
import {afterEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../../shared/i18n";
import HawkAuthEditor,{hawkConfig} from "./HawkAuthEditor";
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
it("preserves templated credentials when enabling payload and delegation fields",()=>{
 vi.stubGlobal("ResizeObserver",class {observe(){} unobserve(){} disconnect(){}});setLanguage("en");
 function Host(){const[config,change]=useState({...hawkConfig(),key:"{{hawk_key}}"});return <Theme><HawkAuthEditor config={config} change={change}/><output data-testid="config">{JSON.stringify(config)}</output></Theme>;}
 render(<Host/>);expect((screen.getByLabelText("Hawk key") as HTMLInputElement).type).toBe("password");fireEvent.click(screen.getByRole("checkbox",{name:"Include the actual payload hash"}));fireEvent.click(screen.getByText("Advanced Hawk parameters"));
 fireEvent.change(screen.getByRole("textbox",{name:"Hawk application ID"}),{target:{value:"application"}});fireEvent.change(screen.getByRole("textbox",{name:"Hawk delegation ID"}),{target:{value:"delegate"}});fireEvent.change(screen.getByRole("textbox",{name:/Hawk timestamp/}),{target:{value:"{{hawk_timestamp}}"}});
 expect(JSON.parse(screen.getByTestId("config").textContent!)).toMatchObject({key:"{{hawk_key}}",include_payload_hash:true,app:"application",delegation:"delegate",timestamp:"{{hawk_timestamp}}"});
});
