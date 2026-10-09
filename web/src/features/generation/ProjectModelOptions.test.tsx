// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import ProjectModelOptions from "./ProjectModelOptions";

afterEach(cleanup);
it("submits real boolean renderer values and an exact selected component name",async()=>{
 await setLanguage("en");const onChange=vi.fn();
 render(<Theme><ProjectModelOptions target={{id:"model-typescript",kind:"model",upstream_stability:"upstream",validation:"fixture",options:{schemaName:"*","runtime-typecheck":true},model_options:[{name:"runtime-typecheck",type:"boolean",description:"Check decoded JSON"}]}} options={{schemaName:"Pet"}} disabled={false} onChange={onChange}/></Theme>);
 fireEvent.click(screen.getByText("Model code style and serialization options"));
 fireEvent.click(screen.getByRole("checkbox",{name:"runtime-typecheck"}));
 expect(onChange).toHaveBeenCalledWith({schemaName:"Pet","runtime-typecheck":false});
 fireEvent.change(screen.getByRole("textbox",{name:"Model name"}),{target:{value:"Response"}});
 expect(onChange).toHaveBeenCalledWith({schemaName:"Response"});
});
