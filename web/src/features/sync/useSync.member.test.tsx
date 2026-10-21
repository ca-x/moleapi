// @vitest-environment jsdom
import {act,cleanup,renderHook,waitFor} from '@testing-library/react';
import {QueryClient,QueryClientProvider} from '@tanstack/react-query';
import {afterEach,expect,it,vi} from 'vitest';
import {api} from '../../shared/api';
import {nativeMemberSession,beginNativeMemberChange} from '../../shared/nativeMemberSession';
import {useSync} from './useSync';
import type {useWorkspace} from '../workspaces/useWorkspace';
vi.mock('../../shared/api',()=>({api:vi.fn(),native:true}));
vi.mock('sonner',()=>({toast:{success:vi.fn(),error:vi.fn(),message:vi.fn()}}));
afterEach(cleanup);
const workspace={} as ReturnType<typeof useWorkspace>;
it('installs a connected public generation and clears it before disconnect while removing remote caches',async()=>{
 const client=new QueryClient({defaultOptions:{queries:{retry:false}}});beginNativeMemberChange();let connected=false;
 vi.mocked(api).mockImplementation((path,method)=>{if(path==='/api/sync/connect'){connected=method!=='DELETE';return Promise.resolve(connected?{connected:true,session_id:'connected-marker',server_url:'https://hosted.test',username:'remote'}:{connected:false});}return Promise.resolve(connected?{connected:true,session_id:'connected-marker',server_url:'https://hosted.test',username:'remote'}:{connected:false});});
 const wrapper=({children}:{children:React.ReactNode})=><QueryClientProvider client={client}>{children}</QueryClientProvider>;
 const {result}=renderHook(()=>useSync(true,workspace,()=>{}),{wrapper});await waitFor(()=>expect(result.current.sync.isSuccess).toBe(true));
 await act(async()=>result.current.connectServer());expect(nativeMemberSession().sessionId).toBe('connected-marker');
 client.setQueryData(['shared-workspace','native-old','o','w'],{private:'old'});client.setQueryData(['workspaces','local'],{keep:'local'});
 await act(async()=>result.current.disconnectServer());expect(nativeMemberSession().sessionId).toBeNull();expect(client.getQueryData(['shared-workspace','native-old','o','w'])).toBeUndefined();expect(client.getQueryData(['workspaces','local'])).toEqual({keep:'local'});client.clear();
});
it('ignores an old status result after a newer connect and does not restore an expired marker',async()=>{
 const client=new QueryClient({defaultOptions:{queries:{retry:false}}});beginNativeMemberChange();let resolve!:(v:unknown)=>void;let statusCalls=0;
 vi.mocked(api).mockImplementation(path=>path==='/api/sync/connect'?Promise.resolve({connected:true,session_id:'new-generation'}):++statusCalls===1?new Promise(done=>{resolve=done;}):Promise.resolve({connected:true,session_id:'new-generation'}));
 const wrapper=({children}:{children:React.ReactNode})=><QueryClientProvider client={client}>{children}</QueryClientProvider>;
 const {result}=renderHook(()=>useSync(true,workspace,()=>{}),{wrapper});await waitFor(()=>expect(statusCalls).toBe(1));await act(async()=>result.current.connectServer());await act(async()=>resolve({connected:true,session_id:'old-generation'}));expect(nativeMemberSession().sessionId).toBe('new-generation');await waitFor(()=>expect(result.current.sync.data?.session_id).toBe('new-generation'));client.clear();
});
