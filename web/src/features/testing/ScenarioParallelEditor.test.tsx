// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import type {Scenario} from "../../shared/types";
import ScenarioParallelEditor from "./ScenarioParallelEditor";
vi.mock("../../shared/ui",()=>({Field:({label,children}:{label:string;children:React.ReactNode})=><label>{label}{children}</label>,Choice:({label,value,options,onChange,disabled}:{label:string;value:string;options:{value:string;label:string}[];onChange:(value:string)=>void;disabled:boolean})=><select aria-label={label} value={value} onChange={event=>onChange(event.target.value)} disabled={disabled}>{options.map(option=><option key={option.value} value={option.value}>{option.label}</option>)}</select>}));
afterEach(cleanup);
const scenario:Scenario={id:"s",name:"Flow",description:"",collection_id:"c",steps:[{id:"a",request_id:"r",name:"First",group:"",enabled:true},{id:"b",request_id:"r",name:"Second",group:"",enabled:true},{id:"c",request_id:"r",name:"Third",group:"",enabled:true}]};
it("creates named consecutive parallel membership and edits its concurrency",async()=>{
 await setLanguage("en");const onChange=vi.fn();const view=render(<Theme><ScenarioParallelEditor scenario={scenario} disabled={false} onChange={onChange}/></Theme>);fireEvent.change(screen.getByRole("textbox",{name:"Parallel block name"}),{target:{value:"Fetch peers"}});fireEvent.change(screen.getByRole("combobox",{name:"Parallel starting step"}),{target:{value:"a"}});fireEvent.click(screen.getByRole("button",{name:"Add parallel block"}));const parallel=onChange.mock.calls[0][0];expect(parallel[0]).toEqual(expect.objectContaining({name:"Fetch peers",step_ids:["a","b"],concurrency:2}));view.rerender(<Theme><ScenarioParallelEditor scenario={{...scenario,parallel}} disabled={false} onChange={onChange}/></Theme>);fireEvent.change(screen.getByRole("combobox",{name:"Block concurrency"}),{target:{value:"4"}});expect(onChange).toHaveBeenLastCalledWith([{...parallel[0],concurrency:4}]);fireEvent.click(screen.getByRole("button",{name:"Remove parallel block"}));expect(onChange).toHaveBeenLastCalledWith([]);
});
it("rejects members with explicit flow redirects without changing the scenario",async()=>{
 await setLanguage("en");const onChange=vi.fn();render(<Theme><ScenarioParallelEditor scenario={{...scenario,steps:[{...scenario.steps[0],on_true:{action:"stop"}},...scenario.steps.slice(1)]}} disabled={false} onChange={onChange}/></Theme>);fireEvent.change(screen.getByRole("textbox",{name:"Parallel block name"}),{target:{value:"Invalid"}});fireEvent.change(screen.getByRole("combobox",{name:"Parallel starting step"}),{target:{value:"a"}});fireEvent.click(screen.getByRole("button",{name:"Add parallel block"}));expect(onChange).not.toHaveBeenCalled();expect(screen.getByRole("alert")).toBeTruthy();
});
