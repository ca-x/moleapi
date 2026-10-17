import {expect,it} from "vitest";
import {openapiMockChoices} from "./openapiMockChoices";

it("lists exact JSON response pointers without requiring operationId",()=>{
  const source=JSON.stringify({openapi:"3.1.0",paths:{"/items/{id}":{get:{responses:{"200":{content:{"application/json":{schema:{type:"object"}}}},"default":{content:{"application/json":{schema:{type:"object"}}}}}}}}});
  expect(openapiMockChoices(source,"GET")).toEqual([{pointer:"#/paths/~1items~1{id}/get/responses/200/content/application~1json/schema",label:"GET /items/{id} · 200 application/json",status:200,mediaType:"application/json"}]);
});

it("lists YAML response references and omits unsafe or non-JSON choices",()=>{
  const source=`openapi: 3.0.3\npaths:\n  /pets:\n    post:\n      responses:\n        '201':\n          $ref: '#/components/responses/Created'\n        '202':\n          $ref: https://example.test/response\ncomponents:\n  responses:\n    Created:\n      content:\n        application/problem+json:\n          schema: {type: object}\n        text/plain:\n          schema: {type: string}\n`;
  expect(openapiMockChoices(source,"POST")).toEqual([{pointer:"#/paths/~1pets/post/responses/201/content/application~1problem+json/schema",label:"POST /pets · 201 application/problem+json",status:201,mediaType:"application/problem+json"}]);
});
