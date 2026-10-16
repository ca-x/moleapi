// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {DataResults} from "./DataResults";
import type {Result} from "./types";
afterEach(cleanup);
const result:Result={id:"query",columns:[{name:"value",data_type:"TEXT",nullable:false}],rows:[[{kind:"text",value:"one"}]],elapsed_ms:1,rows_affected:0,truncated:false,limit_reason:null,done:true};
it("offers an explicit dataset snapshot only when returned rows are complete",()=>{
  const save=vi.fn();const view=render(<DataResults result={{...result,done:false}} onSaveDataset={save}/>);
  expect(screen.queryByRole("button",{name:"保存为数据集"})).toBeNull();
  view.rerender(<DataResults result={{...result,rows:[]}} onSaveDataset={save}/>);
  expect(screen.queryByRole("button",{name:"保存为数据集"})).toBeNull();
  view.rerender(<DataResults result={{...result,failed:true}} onSaveDataset={save}/>);
  expect(screen.queryByRole("button",{name:"保存为数据集"})).toBeNull();
  view.rerender(<DataResults result={result} onSaveDataset={save}/>);
  fireEvent.click(screen.getByRole("button",{name:"保存为数据集"}));
  expect(save).toHaveBeenCalledTimes(1);
});
