// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import {newRequest} from "../../shared/model";
import ExtractionsEditor from "./ExtractionsEditor";
vi.mock("../../shared/ui",()=>({Choice:({value,options,label}:{value:string;options:{value:string;label:string}[];label:string})=><select aria-label={label} value={value} onChange={()=>{}}>{options.map(option=><option key={option.value} value={option.value}>{option.label}</option>)}</select>,ToolButton:({children,label,onClick}:{children:React.ReactNode;label:string;onClick:()=>void})=><button aria-label={label} onClick={onClick}>{children}</button>}));
afterEach(cleanup);
it("creates environment extraction rules with required status and supported sources",async()=>{
 await setLanguage("en");const request=newRequest(),update=vi.fn();const view=render(<Theme><ExtractionsEditor request={request} update={update}/></Theme>);fireEvent.click(screen.getByRole("button",{name:"Add response extraction"}));const rules=update.mock.calls[0][0].extractions;expect(rules[0]).toEqual(expect.objectContaining({kind:"json",scope:"environment",key:"token",required:true,enabled:true}));view.rerender(<Theme><ExtractionsEditor request={{...request,extractions:rules}} update={update}/></Theme>);for(const source of ["JSON Pointer","JSONPath","XPath","Regex capture","Header","Complete response body"])expect(screen.getByRole("option",{name:source})).toBeTruthy();fireEvent.change(screen.getByRole("textbox",{name:"Destination variable name"}),{target:{value:"access_token"}});expect(update).toHaveBeenLastCalledWith({extractions:[{...rules[0],key:"access_token"}]});
});
