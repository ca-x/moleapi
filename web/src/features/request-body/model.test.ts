import {it,expect} from "vitest";
import {bodyModePatch,parseBody} from "./model";
import {newRequest} from "../../shared/model";
it("switching JSON to binary disables stale Content-Type while preserving its original",()=>{const request=newRequest();request.body_kind="json";request.body='{"old":true}';request.headers=[{id:"ct",key:"Content-Type",value:"application/json",enabled:true},{id:"extra",key:"X-Custom",value:"keep",enabled:true}];const patch=bodyModePatch(request,"binary");expect(patch.headers).toEqual([{id:"ct",key:"Content-Type",value:"application/json",enabled:false},request.headers[1]]);expect(parseBody("binary",patch.body!)).toEqual({file_name:"",mime:"",base64:null});});
