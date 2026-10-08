// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {useState} from "react";
import {afterEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../../shared/i18n";
import OAuth1AuthEditor,{oauth1Config} from "./OAuth1AuthEditor";
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
it("keeps credential templates while enabling body hash and empty-parameter signing options",async()=>{
 await setLanguage("en");vi.stubGlobal("ResizeObserver",class{observe(){}unobserve(){}disconnect(){}});
 function Host(){const[config,change]=useState({...oauth1Config(),consumer_secret:"{{consumer_secret}}",private_key:"{{inactive_private_key}}"});return <Theme><OAuth1AuthEditor config={config} change={change}/><output data-testid="config">{JSON.stringify(config)}</output></Theme>;}
 render(<Host/>);expect((screen.getByLabelText("Consumer Secret") as HTMLInputElement).type).toBe("password");
 fireEvent.click(screen.getByRole("checkbox",{name:"Include non-form body hash"}));fireEvent.click(screen.getByRole("checkbox",{name:"Sign empty parameter values"}));fireEvent.click(screen.getByText("Advanced OAuth1 parameters"));
 fireEvent.change(screen.getByLabelText(/OAuth1 timestamp/),{target:{value:"{{timestamp}}"}});
 expect(JSON.parse(screen.getByTestId("config").textContent!)).toMatchObject({consumer_secret:"{{consumer_secret}}",private_key:"{{inactive_private_key}}",include_body_hash:true,include_empty_params:false,timestamp:"{{timestamp}}"});
});
