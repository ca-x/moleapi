import { expect, it } from "vitest";
import Papa from "papaparse";
import { exportResult } from "./export";
import type { Result } from "./types";
const result:Result = {id:"fixture",columns:[{name:"same",data_type:"integer",nullable:false},{name:"same",data_type:"text",nullable:true}],rows:[[{kind:"integer",value:"9007199254740993"},{kind:"text",value:'=SUM(1,2)'}],[{kind:"decimal",value:"1.23000000000000000001"},{kind:"null"}]],elapsed_ms:3,rows_affected:0,truncated:true,limit_reason:"Row cap",done:true};
it("exports typed exact JSON with duplicate names and the truncation reason", () => {
  const parsed=JSON.parse(exportResult(result,"json").content);
  expect(parsed.rows).toEqual(result.rows);
  expect(parsed.columns).toEqual(result.columns);
  expect(parsed.truncated).toBe(true);
  expect(parsed.limit_reason).toBe("Row cap");
});
it("uses mature CSV quoting, exact numeric strings and formula escaping", () => {
  expect(Papa.parse(exportResult(result,"csv").content).data).toEqual([["same","same"],["9007199254740993","'=SUM(1,2)"],["1.23000000000000000001",""]]);
});
