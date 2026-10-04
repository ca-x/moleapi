import { expect, it } from "vitest";
import { resultContents, callbackTemplate } from "./model";
it("unifies SDK tool, resource and prompt content without visiting resource URLs", () => {
  expect(resultContents({content:[{type:"text",text:"tool"}]})).toEqual([{type:"text",text:"tool"}]);
  expect(resultContents({contents:[{uri:"file:///not-read",text:"resource"}]})).toEqual([{type:"resource",resource:{uri:"file:///not-read",text:"resource"}}]);
  expect(resultContents({messages:[{role:"user",content:{type:"text",text:"prompt"}}]})).toEqual([{type:"text",text:"prompt"}]);
});
it("manual callback templates are typed SDK responses and do not invoke an AI model", () => {
  expect(JSON.parse(callbackTemplate("elicitation/create")).action).toBe("accept");
  expect(JSON.parse(callbackTemplate("sampling/createMessage")).model).toBe("manual");
  expect(JSON.parse(callbackTemplate("roots/list"))).toEqual({roots:[]});
});
