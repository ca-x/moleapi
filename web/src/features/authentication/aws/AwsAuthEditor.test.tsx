// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {useState} from "react";
import {afterEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../../shared/i18n";
import AwsAuthEditor,{awsConfig} from "./AwsAuthEditor";
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
it("preserves credential templates while configuring query presigning",()=>{
 vi.stubGlobal("ResizeObserver",class {observe(){} unobserve(){} disconnect(){}});Object.defineProperty(HTMLElement.prototype,"scrollIntoView",{configurable:true,value:vi.fn()});setLanguage("en");
 function Host(){const[config,change]=useState({...awsConfig(),secret_key:"{{aws_secret}}"});return <Theme><AwsAuthEditor config={config} change={change}/><output data-testid="config">{JSON.stringify(config)}</output></Theme>;}
 render(<Host/>);expect((screen.getByLabelText("Secret Access Key") as HTMLInputElement).type).toBe("password");fireEvent.change(screen.getByRole("textbox",{name:"AWS service"}),{target:{value:"s3"}});
 fireEvent.keyDown(screen.getByRole("combobox",{name:"AWS signature placement"}),{key:"Enter"});fireEvent.click(screen.getByRole("option",{name:"Presigned query parameters"}));
 fireEvent.change(screen.getByRole("spinbutton",{name:"AWS presign expiry in seconds"}),{target:{value:"3600"}});
 expect((screen.getByRole("checkbox",{name:"Use UNSIGNED-PAYLOAD"}) as HTMLButtonElement).disabled).toBe(true);expect(screen.getByRole("checkbox",{name:"Use UNSIGNED-PAYLOAD"}).getAttribute("aria-checked")).toBe("true");
 expect(JSON.parse(screen.getByTestId("config").textContent!)).toMatchObject({service:"s3",location:"query",expires_seconds:3600,secret_key:"{{aws_secret}}"});
});
