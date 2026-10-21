import { expect,it } from "vitest";
import { newRequest } from "../../shared/model";
import { savedSubscriptionMatches } from "./sharedGraphQL";
const source={...newRequest(),protocol:{kind:"graphql" as const,document:"subscription Watch($id:Int!){ticks(id:$id)}",variables:{id:1},variables_source:'{"id":1}',connection_params:{},operation_name:null}};
it("matches the same saved subscription despite selected operation name and object key order",()=>{
  expect(savedSubscriptionMatches(source,{...source,protocol:{...source.protocol,operation_name:"Watch"}})).toBe(true);
  expect(savedSubscriptionMatches(source,{...source,protocol:{...source.protocol,operation_name:"Watch",variables:{id:2}}})).toBe(false);
});
it("rejects changed document, ambiguous operation selection and invalid saved variables",()=>{
  expect(savedSubscriptionMatches(source,{...source,protocol:{...source.protocol,document:"subscription Other {ticks}"}})).toBe(false);
  expect(savedSubscriptionMatches({...source,protocol:{...source.protocol,variables_source:"invalid"}},source)).toBe(false);
  const ambiguous={...source,protocol:{...source.protocol,document:"subscription A {ticks} subscription B {ticks}",operation_name:null}};
  expect(savedSubscriptionMatches(ambiguous,{...ambiguous,protocol:{...ambiguous.protocol,operation_name:"B"}})).toBe(false);
});
