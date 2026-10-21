import {expect,it} from 'vitest';
import {beginNativeMemberChange,applyNativeMemberStatus,nativeMemberSession,expireNativeMemberSession} from './nativeMemberSession';
it('does not restore a previous connection from a late status reply or an expired generation',()=>{
 const first=beginNativeMemberChange();expect(applyNativeMemberStatus({connected:true,session_id:'first'},first)).toBe(true);
 const old=nativeMemberSession();const second=beginNativeMemberChange();expect(applyNativeMemberStatus({connected:true,session_id:'new'},second)).toBe(true);
 expect(applyNativeMemberStatus({connected:true,session_id:'first'},old.epoch)).toBe(false);expect(expireNativeMemberSession('first')).toBe(false);expect(nativeMemberSession().sessionId).toBe('new');
 expect(expireNativeMemberSession('new')).toBe(true);const current=nativeMemberSession();expect(applyNativeMemberStatus({connected:true,session_id:'new'},current.epoch)).toBe(false);expect(nativeMemberSession().sessionId).toBeNull();
});
