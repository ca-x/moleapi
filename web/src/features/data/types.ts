export type DataSource = "postgresql" | "mysql" | "local_file" | "remote_file";
export interface DataConfig { kind:"data"; source:DataSource; sql:string; file_name:string; file_format:"csv"|"json"|"parquet"; file_base64:string; table_name:string; csv_header:boolean; read_only:boolean; tls:boolean; ca_pem:string }
export type Cell = {kind:"null"} | {kind:"bool";value:boolean} | {kind:"text"|"integer"|"decimal";value:string} | {kind:"binary";base64:string;bytes:number} | {kind:"truncated";preview:string;bytes:number};
export interface DataColumn {name:string;data_type:string;nullable:boolean}
export interface SchemaColumn extends DataColumn { reference:string }
export interface SchemaTable {schema:string;name:string;reference:string;columns:SchemaColumn[]}
export interface ConnectionInfo {source:DataSource;public_url:string;tls:boolean;tls_verified:boolean;file_bytes:number|null}
export interface Result {id:string;columns:DataColumn[];rows:Cell[][];elapsed_ms:number;rows_affected:number;truncated:boolean;limit_reason:string|null;done:boolean}
export type DataEvent = {kind:"data_ready";info:ConnectionInfo} | {kind:"data_schema";tables:SchemaTable[];clear:boolean;done:boolean;truncated:boolean} | {kind:"data_started";query_id:string} | {kind:"data_columns";query_id:string;columns:DataColumn[]} | {kind:"data_rows";query_id:string;rows:Cell[][]} | {kind:"data_finished";query_id:string;rows_affected:number;elapsed_ms:number;truncated:boolean;limit_reason:string|null} | {kind:"data_error";query_id:string;message:string} | {kind:"data_cancelled";query_id:string;write_outcome_unknown:boolean};
