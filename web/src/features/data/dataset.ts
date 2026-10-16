import type {SavedDataset} from "../../shared/types";
import type {Cell,Result} from "./types";

export type ResultDatasetErrorCode = "incomplete"|"failed"|"truncated"|"empty"|"rows"|"columns"|"duplicate_columns"|"unsupported_cell"|"source_size";
export class ResultDatasetError extends Error {
  constructor(readonly code:ResultDatasetErrorCode){super(code);}
}

function value(cell:Cell):string|boolean|null {
  switch(cell.kind){
    case "null":return null;
    case "bool":return cell.value;
    case "text":case "integer":case "decimal":return cell.value;
    case "binary":case "truncated":throw new ResultDatasetError("unsupported_cell");
  }
}

export function resultDatasetSource(result:Result):NonNullable<SavedDataset["source"]> {
  if(!result.done)throw new ResultDatasetError("incomplete");
  if(result.failed)throw new ResultDatasetError("failed");
  if(result.truncated)throw new ResultDatasetError("truncated");
  if(result.rows.length===0)throw new ResultDatasetError("empty");
  if(result.rows.length>100)throw new ResultDatasetError("rows");
  if(result.columns.length===0||result.columns.length>100)throw new ResultDatasetError("columns");
  const names=result.columns.map(column=>column.name);
  if(names.some(name=>!name.trim())||new Set(names).size!==names.length)throw new ResultDatasetError("duplicate_columns");
  const rows=result.rows.map(cells=>{
    if(cells.length!==names.length)throw new ResultDatasetError("columns");
    const row:Record<string,string|boolean|null>=Object.create(null);
    cells.forEach((cell,index)=>{row[names[index]]=value(cell);});
    return row;
  });
  const source=JSON.stringify(rows);
  if(new TextEncoder().encode(source).length>1024*1024)throw new ResultDatasetError("source_size");
  return {format:"json",source};
}
