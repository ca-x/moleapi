import {it,expect} from "vitest";
import {selectedStatement,formatSql,cellText} from "./model";
it("selects actual SQL statements around quoted semicolons, comments and selections",()=>{
 const document="SELECT ';' AS text;\n-- comment\nSELECT 2 AS n;";
 expect(selectedStatement(document,"postgresql",5)).toBe("SELECT ';' AS text;");
 expect(selectedStatement(document,"postgresql",document.indexOf("SELECT 2")+5)).toContain("SELECT 2 AS n");
 expect(selectedStatement(document,"mysql",0,document.length)).toBe(document);
 expect(formatSql("select n from data where n>1","postgresql")).toContain("SELECT");
});
it("preserves integer precision, null and binary identities for table adapters",()=>{
 expect(cellText({kind:"integer",value:"9007199254740993"})).toBe("9007199254740993");
 expect(cellText({kind:"null"})).toBe("NULL");
 expect(cellText({kind:"binary",base64:"AP8=",bytes:2})).toBe("AP8=");
});
