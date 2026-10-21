// @vitest-environment jsdom
import {act,cleanup,fireEvent,render,screen} from '@testing-library/react';
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {setLanguage} from '../../shared/i18n';
import {newRequest} from '../../shared/model';
import {a2aConfig} from './model';
import A2aCardEditor from './A2aCardEditor';
import type {A2aCardResult} from './types';
vi.mock('../../shared/ui',()=>({Editor:({value,onChange,label}:{value:string;onChange:(v:string)=>void;label:string})=><textarea aria-label={label} value={value} onChange={e=>onChange(e.target.value)}/>,Field:({label,children}:{label:string;children:React.ReactNode})=><label>{label}{children}</label>}));
beforeEach(async()=>{await setLanguage('zh-CN');});afterEach(cleanup);
const candidate={specification:{id:'card',name:'Agent',kind:'a2a-agent-card',dialect:'a2a-0.3',source:'{}'},card:{name:'Agent'},interfaces:[],warnings:[],dialect:'0.3'} as A2aCardResult;
function props(){return {scope:'first',open:true,onOpenChange:vi.fn(),request:{...newRequest(),url:'http://fixture.test',protocol:a2aConfig()},dark:false,isCurrent:()=>true,onAttach:vi.fn(),importCard:vi.fn().mockResolvedValue(candidate),discoverCard:vi.fn().mockResolvedValue(candidate),cancelDiscovery:vi.fn().mockResolvedValue(undefined)};}
it('Stop cancels discovery and discards a late candidate',async()=>{const p=props();let resolve!:(v:A2aCardResult)=>void;p.discoverCard=vi.fn(()=>new Promise(done=>{resolve=done;}));render(<A2aCardEditor {...p}/>);fireEvent.click(screen.getByRole('button',{name:'发现 Agent Card'}));fireEvent.click(screen.getByRole('button',{name:'停止发现'}));await act(async()=>resolve(candidate));expect(p.cancelDiscovery).toHaveBeenCalledWith(expect.any(String));expect(p.onAttach).not.toHaveBeenCalled();});
it('editing source while validation is pending discards the result',async()=>{const p=props();let resolve!:(v:A2aCardResult)=>void;p.importCard=vi.fn(()=>new Promise(done=>{resolve=done;}));render(<A2aCardEditor {...p}/>);fireEvent.click(screen.getByRole('button',{name:'验证并使用定义'}));fireEvent.change(screen.getByLabelText('Agent Card 原始 JSON'),{target:{value:'{"changed":true}'}});await act(async()=>resolve(candidate));expect(p.onAttach).not.toHaveBeenCalled();});
it('reopened dialogs and replaced requests cannot attach an earlier candidate',async()=>{const p=props();let resolve!:(v:A2aCardResult)=>void;p.importCard=vi.fn(()=>new Promise(done=>{resolve=done;}));const{rerender}=render(<A2aCardEditor {...p}/>);fireEvent.click(screen.getByRole('button',{name:'验证并使用定义'}));rerender(<A2aCardEditor {...p} open={false}/>);rerender(<A2aCardEditor {...p} request={{...p.request,name:'changed'}}/>);await act(async()=>resolve(candidate));expect(p.onAttach).not.toHaveBeenCalled();});
