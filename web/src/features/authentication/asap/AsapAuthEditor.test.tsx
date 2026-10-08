// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {useState} from "react";
import {afterEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../../shared/i18n";
import AsapAuthEditor,{asapConfig} from "./AsapAuthEditor";
vi.mock("../../../shared/ui",async()=>{const actual=await vi.importActual<object>("../../../shared/ui");return {...actual,Editor:({value,label}:{value:string;label:string})=><textarea aria-label={label} value={value} readOnly/>};});
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
it("keeps key and JSON templates while changing audience and lifetime fields",async()=>{
 await setLanguage("en");vi.stubGlobal("ResizeObserver",class{observe(){}unobserve(){}disconnect(){}});
 function Host(){const[config,change]=useState({...asapConfig(),private_key:"{{private_key}}",claims_source:'{"custom":"{{value}}"}'});return <Theme><AsapAuthEditor config={config} change={change} dark={false}/><output data-testid="config">{JSON.stringify(config)}</output></Theme>;}
 render(<Host/>);fireEvent.change(screen.getByLabelText(/Audience \/ aud/),{target:{value:"first,second"}});fireEvent.change(screen.getByLabelText("ASAP lifetime in seconds"),{target:{value:"120"}});expect(JSON.parse(screen.getByTestId("config").textContent!)).toMatchObject({private_key:"{{private_key}}",claims_source:'{"custom":"{{value}}"}',audience:["first","second"],ttl_seconds:120});expect(screen.getByRole("combobox",{name:"ASAP signing algorithm"})).toBeTruthy();
});
