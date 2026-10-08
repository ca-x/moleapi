import { expect, it } from "vitest";
import { newRequest } from "../../shared/model";
import { authenticationEligibility } from "./eligibility";
it("allows API keys and JWT for HTTP handshakes but Digest only for finite requests", () => {
  const request = newRequest("auth", "https://example.test");
  for (const kind of ["sse", "websocket", "grpc", "socketio", "a2a"] as const) {
    const candidate = {...request, protocol:{kind}} as typeof request;
    expect(authenticationEligibility(candidate)).toEqual({api:true,digest:false,signing:false,query:kind!=="grpc"});
  }
  expect(authenticationEligibility(request)).toEqual({api:true,digest:true,signing:true,query:true});
});
it("uses the selected GraphQL operation to determine Digest eligibility", () => {
  const request = newRequest("auth", "https://example.test");
  const protocol = {kind:"graphql" as const, document:"query Read {value} subscription Watch {value}",variables:{},connection_params:{},operation_name:"Read"};
  expect(authenticationEligibility({...request,protocol})).toEqual({api:true,digest:true,signing:true,query:true});
  expect(authenticationEligibility({...request,protocol:{...protocol,operation_name:"Watch"}})).toEqual({api:true,digest:false,signing:false,query:true});
  expect(authenticationEligibility({...request,protocol:{...protocol,operation_name:null}}).digest).toBe(false);
});
it("distinguishes remote Data HTTP from database and local transports", () => {
  const request = newRequest("auth", "https://example.test");
  for (const source of ["local_file", "postgresql", "mysql", "remote_file"] as const) {
    const candidate = {...request,protocol:{kind:"data",source}} as typeof request;
    expect(authenticationEligibility(candidate)).toEqual({api:source==="remote_file",digest:false,signing:false,query:true});
  }
  for (const kind of ["tcp", "mqtt"] as const) {
    expect(authenticationEligibility({...request,protocol:{kind}} as typeof request).api).toBe(false);
  }
  expect(authenticationEligibility({...request,protocol:{kind:"mcp",transport:"stdio"}} as typeof request).api).toBe(false);
});
