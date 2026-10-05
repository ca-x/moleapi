// @vitest-environment jsdom
import {act,cleanup,renderHook} from "@testing-library/react";
import {afterEach,expect,it} from "vitest";
import {useDataEvents} from "./useDataEvents";
import type {ProtocolEvent} from "../protocols/types";
afterEach(cleanup);
function event(cursor:number,message:ProtocolEvent["message"]):ProtocolEvent {return {cursor,received_at:"fixture",direction:"incoming",message};}
it("ignores previous query cancellation and error after reserving another query",()=>{
  let events=[event(1,{kind:"data_started",query_id:"first"}),event(2,{kind:"data_finished",query_id:"first",rows_affected:0,elapsed_ms:1,truncated:false,limit_reason:null})];
  const {result,rerender}=renderHook(()=>useDataEvents("session",events,0));
  act(()=>{result.current.reserve("second");});
  events=[...events,event(3,{kind:"data_started",query_id:"second"}),event(4,{kind:"data_columns",query_id:"second",columns:[{name:"n",data_type:"BIGINT",nullable:false}]}),event(5,{kind:"data_rows",query_id:"second",rows:[[{kind:"integer",value:"9007199254740993"}]]}),event(6,{kind:"data_cancelled",query_id:"first",write_outcome_unknown:true}),event(7,{kind:"data_error",query_id:"first",message:"old failure"})];
  rerender();
  expect(result.current.active).toBe("second");
  expect(result.current.error).toBe("");
  expect(result.current.result?.rows[0][0]).toEqual({kind:"integer",value:"9007199254740993"});
});
it("keeps incomplete schema marked after a cursor gap until a fresh clear batch",()=>{
  let events=[event(1,{kind:"data_schema",tables:[],clear:true,done:false,truncated:false})];
  const {result,rerender}=renderHook(()=>useDataEvents("session",events,0));
  events=[event(4,{kind:"data_schema",tables:[],clear:false,done:true,truncated:false})];rerender();
  expect(result.current.schemaTruncated).toBe(true);
  events=[...events,event(5,{kind:"data_schema",tables:[],clear:true,done:true,truncated:false})];rerender();
  expect(result.current.schemaTruncated).toBe(false);
});
it("clears query state on a session change instead of displaying prior rows",()=>{
  let session="first";let events=[event(1,{kind:"data_started",query_id:"query"})];
  const {result,rerender}=renderHook(()=>useDataEvents(session,events,0));
  expect(result.current.active).toBe("query");
  session="second";events=[];rerender();
  expect(result.current.active).toBe(null);
  expect(result.current.result).toBe(null);
});

it("clears an unfinished query when a disconnect emits only a terminal state",()=>{
  let events=[event(1,{kind:"data_started",query_id:"query"})];
  const {result,rerender}=renderHook(()=>useDataEvents("session",events,0));
  events=[...events,event(2,{kind:"state",state:"closed",reason:"Stopped"})];rerender();
  expect(result.current.active).toBe(null);expect(result.current.result).toBe(null);
  expect(result.current.error).not.toBe("");
});
