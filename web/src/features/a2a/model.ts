import { id } from "../../shared/model";
import type { A2aConfig, A2aDialect, A2aMethod } from "./types";
export function paramsTemplate(dialect: A2aDialect, method: A2aMethod): string {
  if (method === "message/send" || method === "message/stream") return JSON.stringify(dialect === "0.3" ? {
    message:{kind:"message",role:"user",messageId:id(),parts:[{kind:"text",text:"Hello"}]},
    configuration:{blocking:true,historyLength:20,acceptedOutputModes:["text/plain"]},
  } : {
    message:{role:"ROLE_USER",messageId:id(),parts:[{text:"Hello",mediaType:"text/plain"}]},
    configuration:{returnImmediately:false,historyLength:20,acceptedOutputModes:["text/plain"]},
  },null,2);
  if (method === "tasks/list") return '{\n  "contextId": "",\n  "pageSize": 20\n}';
  if (method === "tasks/get") return '{\n  "id": "",\n  "historyLength": 20\n}';
  if (method === "tasks/cancel" || method === "tasks/resubscribe") return '{\n  "id": ""\n}';
  if (method === "tasks/pushNotificationConfig/set") return JSON.stringify(dialect === "0.3" ? {taskId:"",pushNotificationConfig:{url:"https://example.com/webhook"}} : {taskId:"",id:"",url:"https://example.com/webhook"},null,2);
  return JSON.stringify(dialect === "0.3" ? {id:""} : {taskId:"",...(method !== "tasks/pushNotificationConfig/list" ? {id:""} : {pageSize:20})},null,2);
}
export function a2aConfig(): A2aConfig {
  return { kind:"a2a",dialect:"0.3",transport:"jsonrpc",operation:"message/send",params_source:paramsTemplate("0.3","message/send") };
}

export function interfaceDialect(version: string): A2aDialect | null {
  if (/^0\.3(?:\.\d+)?$/.test(version)) return "0.3";
  if (/^1\.0(?:\.\d+)?$/.test(version)) return "1.0";
  return null;
}
