import { expect, it } from "vitest";
import { completionSchema, syntaxDiagnostics } from "./editor";
it("keeps identically named tables in separate schemas", () => {
  const tables = ["public", "billing"].map(schema => ({schema, name:"items",reference:`${schema}.items`,columns:[{name:schema === "public" ? "id" : "amount",data_type:"integer",nullable:false,reference:""}]}));
  expect(completionSchema(tables)).toEqual({public:{items:["id"]},billing:{items:["amount"]}});
});
it("marks unmatched SQL grouping without treating quoted parentheses as syntax", () => {
  expect(syntaxDiagnostics("SELECT (1", "postgresql", "Grouping error").length).toBeGreaterThan(0);
  expect(syntaxDiagnostics("SELECT '(' AS value", "postgresql", "Grouping error")).toEqual([]);
});
