// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {useState} from "react";
import {afterEach,expect,it} from "vitest";
import {setLanguage} from "../../../shared/i18n";
import NtlmAuthEditor,{ntlmConfig} from "./NtlmAuthEditor";
afterEach(cleanup);
it("preserves templated domain/workstation when changing the explicit binding policy",async()=>{
 await setLanguage("en");function Host(){const[config,change]=useState({...ntlmConfig(),domain:"{{domain}}",workstation:"{{machine}}"});return <Theme><NtlmAuthEditor config={config} change={change}/><output data-testid="value">{JSON.stringify(config)}</output></Theme>;}
 render(<Host/>);fireEvent.click(screen.getByRole("checkbox",{name:"Use TLS channel binding for HTTPS"}));expect(JSON.parse(screen.getByTestId("value").textContent!)).toEqual({domain:"{{domain}}",workstation:"{{machine}}",channel_binding:false});
});
