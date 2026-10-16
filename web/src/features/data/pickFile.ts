import {pickBinaryFile} from "../../shared/pickBinaryFile";
export async function pickDataFile():Promise<{name:string;base64:string}|null> {
  return pickBinaryFile({extensions:["csv","json","jsonl","ndjson","parquet","xlsx"],accept:".csv,.json,.jsonl,.ndjson,.parquet,.xlsx"});
}
