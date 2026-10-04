import { expect, it } from "vitest";
import { paramsTemplate } from "./model";
it("keeps legacy and current SDK message representations separate",()=>{
  const legacy=JSON.parse(paramsTemplate("0.3","message/send"));
  const current=JSON.parse(paramsTemplate("1.0","message/send"));
  expect(legacy.message.role).toBe("user");expect(legacy.message.parts[0].kind).toBe("text");expect(legacy.configuration.blocking).toBe(true);
  expect(current.message.role).toBe("ROLE_USER");expect(current.message.parts[0]).toEqual({text:"Hello",mediaType:"text/plain"});expect(current.configuration.returnImmediately).toBe(false);
});
it("uses supported SDK push parameter shapes without registering a destination",()=>{
  expect(JSON.parse(paramsTemplate("0.3","tasks/pushNotificationConfig/set")).pushNotificationConfig).toEqual({url:"https://example.com/webhook"});
  expect(JSON.parse(paramsTemplate("1.0","tasks/pushNotificationConfig/set")).url).toBe("https://example.com/webhook");
  expect(JSON.parse(paramsTemplate("0.3","tasks/get"))).toEqual({id:"",historyLength:20});
});
it("normalizes advertised patch versions into supported configuration dialects",async()=>{
  const {interfaceDialect}=await import("./model");
  expect(interfaceDialect("0.3.0")).toBe("0.3");expect(interfaceDialect("1.0.0")).toBe("1.0");expect(interfaceDialect("2.0.0")).toBeNull();
});
