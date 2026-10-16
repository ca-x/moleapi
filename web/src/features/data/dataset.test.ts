import {expect,it} from "vitest";
import {resultDatasetSource,ResultDatasetError} from "./dataset";
import type {Result,Cell} from "./types";

function result(columns=["id","amount","active","empty","name"],rows:Cell[][]=[[{kind:"integer",value:"9007199254740993"},{kind:"decimal",value:"1.2300"},{kind:"bool",value:true},{kind:"null"},{kind:"text",value:"猫"}]]):Result {
  return {id:"query",columns:columns.map(name=>({name,data_type:"TEXT",nullable:true})),rows,elapsed_ms:1,rows_affected:0,truncated:false,limit_reason:null,done:true};
}
function code(value:Result){try{resultDatasetSource(value);return "saved";}catch(error){expect(error).toBeInstanceOf(ResultDatasetError);return (error as ResultDatasetError).code;}}

it("snapshots exact typed rows as runner JSON without rounding integers or decimals",()=>{
  const source=resultDatasetSource(result());
  expect(source.format).toBe("json");
  expect(JSON.parse(source.source)).toEqual([{id:"9007199254740993",amount:"1.2300",active:true,empty:null,name:"猫"}]);
});
it("rejects incomplete, truncated, empty and ambiguous results",()=>{
  expect(code({...result(),done:false})).toBe("incomplete");
  expect(code({...result(),truncated:true})).toBe("truncated");
  expect(code({...result(),rows:[]})).toBe("empty");
  expect(code(result(["id","id"],[[{kind:"integer",value:"1"},{kind:"integer",value:"2"}]]))).toBe("duplicate_columns");
  expect(code(result([" "],[[{kind:"text",value:"x"}]]))).toBe("duplicate_columns");
});
it("rejects binary, capped cells and schema/size limits without partial rows",()=>{
  expect(code(result(["payload"],[[{kind:"binary",base64:"AA==",bytes:1}]]))).toBe("unsupported_cell");
  expect(code(result(["preview"],[[{kind:"truncated",preview:"x",bytes:100}]]))).toBe("unsupported_cell");
  expect(code(result(["value"],Array.from({length:101},()=>[{kind:"text",value:"x"}])))).toBe("rows");
  expect(code(result(Array.from({length:101},(_,i)=>`c${i}`),[Array.from({length:101},()=>({kind:"null"}))]))).toBe("columns");
  expect(code(result(["value"],[[{kind:"text",value:"x".repeat(1024*1024)}]]))).toBe("source_size");
});
