// @vitest-environment jsdom
import {beforeEach,expect,it,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {api} from './api';
import {beginNativeMemberChange,applyNativeMemberStatus,nativeMemberSession} from './nativeMemberSession';
import {memberApi,memberCredential,previewMemberMock,closeMemberSession} from './memberApi';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
vi.mock('./api',async original=>({...await original<typeof import('./api')>(),native:true,api:vi.fn(),token:()=>null}));
function connect(id:string){const epoch=beginNativeMemberChange();applyNativeMemberStatus({connected:true,session_id:id},epoch);}
let sequence=0,sessionId='';
beforeEach(()=>{connect(sessionId='fixture-'+ ++sequence);vi.mocked(invoke).mockReset();vi.mocked(api).mockReset();});
it('uses the native member envelope with a public session marker and never invokes owned paths',async()=>{
 vi.mocked(invoke).mockResolvedValue({status:200,body:{status:200,body:{name:'Canonical'},mock_preview:false}});
 expect(memberCredential()).toContain(sessionId);expect(await memberApi('/api/owners/o/workspaces/w')).toEqual({name:'Canonical'});
 expect(invoke).toHaveBeenCalledExactlyOnceWith('api',{method:'POST',path:'/api/sync/collaboration',body:{session_id:sessionId,method:'GET',path:'/api/owners/o/workspaces/w'}});expect(api).not.toHaveBeenCalled();
});
it('preserves simulated Mock401 as data and expires only real current hosted401',async()=>{
 vi.mocked(invoke).mockResolvedValueOnce({status:200,body:{status:401,body:'<script>inert</script>',mock_preview:true}});
 expect(await previewMemberMock('/api/owners/o/workspaces/w/mock/r/e')).toEqual({status:401,body:'<script>inert</script>'});expect(nativeMemberSession().sessionId).toBe(sessionId);
 vi.mocked(invoke).mockResolvedValueOnce({status:200,body:{status:401,body:{error:'expired'},mock_preview:false}});await expect(memberApi('/api/owners/o/workspaces/w')).rejects.toMatchObject({status:401});expect(nativeMemberSession().sessionId).toBeNull();
});
it.each([200,401])('discards a late%s after reconnect without expiring the new session',async(status)=>{
 let resolve!:(v:unknown)=>void;vi.mocked(invoke).mockImplementation(()=>new Promise(done=>{resolve=done;}));const pending=memberApi('/api/owners/o/workspaces/w');await vi.waitFor(()=>expect(invoke).toHaveBeenCalled());connect('next');resolve({status:200,body:{status,body:{name:'stale'},mock_preview:false}});await expect(pending).rejects.toMatchObject({status:409});expect(nativeMemberSession().sessionId).toBe('next');
});
it('rejects before dispatch when disconnected',async()=>{beginNativeMemberChange();await expect(memberApi('/api/owners/o/workspaces/w')).rejects.toMatchObject({status:409});expect(invoke).not.toHaveBeenCalled();});

it('closes a captured native member session through the bridge and rejects another connection marker',async()=>{vi.mocked(invoke).mockResolvedValue({status:200,body:{status:200,body:{},mock_preview:false}});const credential=memberCredential();await closeMemberSession('/api/owners/o/workspaces/w/sessions/s',credential);expect(invoke).toHaveBeenCalledWith('api',expect.objectContaining({body:expect.objectContaining({path:'/api/owners/o/workspaces/w/sessions/s/close',method:'POST'})}));vi.mocked(invoke).mockClear();connect('new-connection-for-close');await closeMemberSession('/api/owners/o/workspaces/w/sessions/s',credential);expect(invoke).not.toHaveBeenCalled();});
