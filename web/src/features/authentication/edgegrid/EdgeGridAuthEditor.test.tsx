// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {useState} from "react";
import {afterEach,expect,it} from "vitest";
import {setLanguage} from "../../../shared/i18n";
import EdgeGridAuthEditor,{edgeGridConfig} from "./EdgeGridAuthEditor";
afterEach(cleanup);
it("keeps credential templates and ordered header names while configuring body-prefix hashing",async()=>{
 await setLanguage("en");function Host(){const[config,change]=useState({...edgeGridConfig(),client_secret:"{{key}}"});return <Theme><EdgeGridAuthEditor config={config} change={change}/><output data-testid="config">{JSON.stringify(config)}</output></Theme>;}
 render(<Host/>);expect((screen.getByLabelText("Client Secret") as HTMLInputElement).type).toBe("password");fireEvent.click(screen.getByText("Advanced EdgeGrid settings"));fireEvent.change(screen.getByLabelText(/Header names to sign/),{target:{value:"X-Z,X-A"}});fireEvent.change(screen.getByLabelText(/EdgeGrid maximum hashed bytes/),{target:{value:"3"}});expect(JSON.parse(screen.getByTestId("config").textContent!)).toMatchObject({client_secret:"{{key}}",headers_to_sign:["X-Z","X-A"],max_body_bytes:3});
});
