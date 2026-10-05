import { PostgreSQL, MySQL, StandardSQL } from "@codemirror/lang-sql";
import { format } from "sql-formatter";
import type { DataConfig, DataSource, Cell } from "./types";
export function dataConfig():DataConfig {return {kind:"data",source:"postgresql",sql:"SELECT 1 AS value",file_name:"",file_format:"csv",file_base64:"",table_name:"data",csv_header:true,read_only:true,tls:true,ca_pem:""};}
export function dialect(source:DataSource) {return source==="postgresql"?PostgreSQL:source==="mysql"?MySQL:StandardSQL;}
/** Lezer owns SQL comments, quotes, statement boundaries, and cursor locations. */
export function selectedStatement(document:string,source:DataSource,from:number,to=from):string {
  if(from!==to)return document.slice(Math.min(from,to),Math.max(from,to)).trim();
  const statements=dialect(source).language.parser.parse(document).topNode.getChildren("Statement");
  const current=statements.find(node=>node.from<=from&&from<=node.to) ?? statements.find(node=>node.from>=from) ?? statements.at(-1);
  return current?document.slice(current.from,current.to).trim():"";
}
export function formatSql(document:string,source:DataSource):string {return format(document,{language:source==="postgresql"?"postgresql":source==="mysql"?"mysql":"sql",keywordCase:"upper"});}
export function cellText(cell:Cell):string {switch(cell.kind){case "null":return "NULL";case "bool":return cell.value?"true":"false";case "binary":return cell.base64;case "truncated":return cell.preview;default:return cell.value;}}
