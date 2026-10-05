import {z} from "zod";
const fileSchema=z.object({file_name:z.string().default(""),mime:z.string().default(""),base64:z.string().nullable().default(null)}).strict();
const partSchema=z.object({id:z.string(),name:z.string(),enabled:z.boolean().default(true),value:z.discriminatedUnion("kind",[z.object({kind:z.literal("text"),text:z.string(),mime:z.string().default("")}).strict(),z.object({kind:z.literal("file"),file:fileSchema}).strict()])}).strict();
export const multipartSchema=z.object({parts:z.array(partSchema).max(64).default([])}).strict();
export type BodyFile=z.infer<typeof fileSchema>;
export type BodyPart=z.infer<typeof partSchema>;
export type MultipartBody=z.infer<typeof multipartSchema>;
export const emptyFile=():BodyFile=>({file_name:"",mime:"",base64:null});
export const initialBody=(kind:string)=>JSON.stringify(kind==="binary"?emptyFile():{parts:[]});
export function parseBody(kind:string,source:string):BodyFile|MultipartBody{return kind==="binary"?fileSchema.parse(JSON.parse(source)):multipartSchema.parse(JSON.parse(source));}

export function bodyModePatch(request:import("../../shared/types").RequestSpec,kind:import("../../shared/types").RequestSpec["body_kind"]):Partial<import("../../shared/types").RequestSpec>{
 return {body_kind:kind,body:["binary","multipart"].includes(kind)?initialBody(kind):["binary","multipart"].includes(request.body_kind)?"":request.body,headers:["binary","multipart"].includes(kind)?request.headers.map(h=>h.key.toLowerCase()==="content-type"?{...h,enabled:false}:h):request.headers};
}
