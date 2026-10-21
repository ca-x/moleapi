// @vitest-environment jsdom
import {act,cleanup,fireEvent,render,screen} from '@testing-library/react';
import {Theme} from '@radix-ui/themes';
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {setLanguage} from '../../shared/i18n';
import {memberApi,memberCredential} from '../../shared/memberApi';
import NativeInvitationForm from './NativeInvitationForm';
vi.mock('../../shared/memberApi',()=>({memberApi:vi.fn(),memberCredential:vi.fn()}));
beforeEach(async()=>{await setLanguage('zh-CN');vi.mocked(memberApi).mockReset();vi.mocked(memberCredential).mockReturnValue('member-marker');});afterEach(cleanup);
const secret='wi_'+ 'a'.repeat(64),link='https://hosted.test/#workspace_invite='+secret;
const props=()=>({scope:'remote-first',serverUrl:'https://hosted.test/',username:'member',onJoined:vi.fn()});
it('accepts only a link for the connected server, clears its one-time token and refreshes the member list',async()=>{const p=props();vi.mocked(memberApi).mockResolvedValue({});render(<Theme><NativeInvitationForm {...p}/></Theme>);fireEvent.change(screen.getByLabelText('托管邀请链接'),{target:{value:link}});await act(async()=>fireEvent.click(screen.getByRole('button',{name:'接受托管邀请'})));expect(memberApi).toHaveBeenCalledExactlyOnceWith('/api/workspace-invitations/accept','POST',{token:secret});expect((screen.getByLabelText('托管邀请链接')as HTMLTextAreaElement).value).toBe('');expect(p.onJoined).toHaveBeenCalledOnce();});
it('rejects an invitation URL from another server before dispatch',async()=>{render(<Theme><NativeInvitationForm {...props()}/></Theme>);fireEvent.change(screen.getByLabelText('托管邀请链接'),{target:{value:link.replace('hosted.test','foreign.test')}});await act(async()=>fireEvent.click(screen.getByRole('button',{name:'接受托管邀请'})));expect(memberApi).not.toHaveBeenCalled();expect(screen.getByRole('alert')).toBeTruthy();});
it.each(['scope','input','credential','unmount'])('withholds late invitation completion after %s changes',async(change)=>{const p=props();let done!:(v:unknown)=>void;vi.mocked(memberApi).mockImplementation(()=>new Promise(resolve=>{done=resolve;}));const {rerender,unmount}=render(<Theme><NativeInvitationForm {...p}/></Theme>);fireEvent.change(screen.getByLabelText('托管邀请链接'),{target:{value:link}});fireEvent.click(screen.getByRole('button',{name:'接受托管邀请'}));if(change==='scope')rerender(<Theme><NativeInvitationForm {...p} scope='remote-next'/></Theme>);if(change==='input')fireEvent.change(screen.getByLabelText('托管邀请链接'),{target:{value:link.replace('aaaa','bbbb')}});if(change==='credential')vi.mocked(memberCredential).mockReturnValue('changed');if(change==='unmount')unmount();await act(async()=>done({}));expect(p.onJoined).not.toHaveBeenCalled();});
