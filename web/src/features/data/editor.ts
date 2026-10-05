import type { Diagnostic } from "@codemirror/lint";
import type { SQLNamespace } from "@codemirror/lang-sql";
import type { DataSource, SchemaTable } from "./types";
import { dialect } from "./model";

/** Preserve database namespaces instead of overwriting identically named tables. */
export function completionSchema(tables: SchemaTable[]): SQLNamespace {
  const schema: Record<string, SQLNamespace> = Object.create(null);
  for (const table of tables) {
    const columns = table.columns.map(column => column.name);
    if (!table.schema) schema[table.name] = columns;
    else {
      const namespace = (schema[table.schema] ??= Object.create(null)) as Record<string, SQLNamespace>;
      namespace[table.name] = columns;
    }
  }
  return schema;
}

/** Lezer diagnoses lexical/grouping errors; the database owns full SQL validation. */
export function syntaxDiagnostics(document: string, source: DataSource, message: string): Diagnostic[] {
  const diagnostics: Diagnostic[] = [];
  dialect(source).language.parser.parse(document).iterate({
    enter(node) {
      if (node.type.isError) diagnostics.push({
        from: node.from,
        to: Math.min(document.length, Math.max(node.to, node.from + 1)),
        severity: "error", message,
      });
    },
  });
  return diagnostics;
}
