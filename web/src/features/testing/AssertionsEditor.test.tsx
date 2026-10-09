// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import {newRequest} from "../../shared/model";
import AssertionsEditor from "./AssertionsEditor";
vi.mock("../../shared/ui",()=>({Choice:({value,onChange,options,label}:{value:string;onChange:(value:string)=>void;options:{value:string;label:string}[];label:string})=><select aria-label={label} value={value} onChange={event=>onChange(event.target.value)}>{options.map(option=><option key={option.value} value={option.value}>{option.label}</option>)}</select>,ToolButton:({children,label,onClick}:{children:React.ReactNode;label:string;onClick:()=>void})=><button aria-label={label} onClick={onClick}>{children}</button>,Editor:({value,onChange,label}:{value:string;onChange:(value:string)=>void;label:string})=><textarea aria-label={label} value={value} onChange={event=>onChange(event.target.value)}/>}));
afterEach(cleanup);
it("provides mature structured assertion kinds and a JSON schema editor",async()=>{
 await setLanguage("en");const request=newRequest();request.assertions=[{id:"a",name:"Schema",kind:"schema",target:"",expected:"{}"}];const update=vi.fn();render(<Theme><AssertionsEditor request={request} update={update} dark/></Theme>);
 for(const kind of ["Response header equals","Regex match / capture","JSONPath value equals","XPath value equals","Validate JSON Schema"])expect(screen.getByRole("option",{name:kind})).toBeTruthy();fireEvent.change(screen.getByRole("textbox",{name:"Assertion JSON Schema"}),{target:{value:'{"type":"object"}'}});expect(update).toHaveBeenCalledWith({assertions:[{...request.assertions[0],expected:'{"type":"object"}'}]});
});
