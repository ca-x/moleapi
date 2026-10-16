// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {Theme} from "@radix-ui/themes";
import {useWorkbench} from "../workbench/context";
import InterchangeFields from "./InterchangeFields";

vi.mock("../workbench/context",()=>({useWorkbench:vi.fn()}));
vi.stubGlobal("ResizeObserver",class{observe(){}unobserve(){}disconnect(){}});
Element.prototype.scrollIntoView=vi.fn();
afterEach(()=>{cleanup();vi.mocked(useWorkbench).mockReset();});

it("offers HAR import without claiming HAR export",()=>{
  const setFormat=vi.fn();
  const state={modal:"import",format:"openapi",setFormat,includeSecrets:false,setIncludeSecrets:vi.fn(),content:"",setContent:vi.fn(),setModalError:vi.fn()};
  vi.mocked(useWorkbench).mockReturnValue(state as unknown as ReturnType<typeof useWorkbench>);
  const view=render(<Theme><InterchangeFields/></Theme>);
  fireEvent.click(screen.getByRole("combobox",{name:"导入导出格式"}));
  fireEvent.click(screen.getByRole("option",{name:"HAR 1.2 JSON"}));
  expect(setFormat).toHaveBeenCalledWith("har");
  vi.mocked(useWorkbench).mockReturnValue({...state,modal:"export"} as unknown as ReturnType<typeof useWorkbench>);
  view.rerender(<Theme><InterchangeFields/></Theme>);
  fireEvent.click(screen.getByRole("combobox",{name:"导入导出格式"}));
  expect(screen.queryByRole("option",{name:"HAR 1.2 JSON"})).toBeNull();
});
