import Papa from "papaparse";
import type { Result } from "./types";
import { cellText } from "./model";
import type { ExportResult } from "../../shared/types";

/** JSON preserves duplicate column names, null/binary types and exact number strings. */
export function exportResult(result: Result, format: "json" | "csv"): ExportResult {
  if (format === "json") return {
    filename: "query-result.json", mime: "application/json",
    content: JSON.stringify({ format: "moleapi.data.result.v1", ...result }, null, 2),
  };
  return {
    filename: "query-result.csv", mime: "text/csv;charset=utf-8",
    content: Papa.unparse([
      result.columns.map(column => column.name),
      ...result.rows.map(row => row.map(cell => cell.kind === "null" ? "" : cellText(cell))),
    ], { escapeFormulae: true }),
  };
}
