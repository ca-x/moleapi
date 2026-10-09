// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import TemplateOutputEditor from "./TemplateOutputEditor";
beforeEach(async()=>{await setLanguage("en");vi.stubGlobal("ResizeObserver",class{observe(){}unobserve(){}disconnect(){}});});
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
it("applies supporting folder/name edits without replacing the template source",()=>{
 const apply=vi.fn();render(<Theme><TemplateOutputEditor source="extra.mustache" value={{templateType:"SupportingFiles",folder:"docs",destinationFilename:"old.md"}} disabled={false} onApply={apply}/></Theme>);
 fireEvent.change(screen.getByRole("textbox",{name:"Output filename / suffix"}),{target:{value:"Guide.md"}});fireEvent.change(screen.getByRole("textbox",{name:"Supporting file output folder"}),{target:{value:"extras"}});fireEvent.click(screen.getByRole("button",{name:"Apply output mapping"}));
 expect(apply).toHaveBeenCalledWith({templateType:"SupportingFiles",folder:"extras",destinationFilename:"Guide.md"});
});
it("reports rejected output mappings without applying an unsafe destination",()=>{
 render(<Theme><TemplateOutputEditor source="extra.mustache" value={{templateType:"Model",destinationFilename:".txt"}} disabled={false} onApply={()=>{throw new Error("invalid mapping");}}/></Theme>);
 fireEvent.click(screen.getByRole("button",{name:"Apply output mapping"}));expect(screen.getByRole("alert").textContent).toContain("Invalid output mapping");
});
