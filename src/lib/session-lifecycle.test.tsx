import { beforeAll, afterEach, afterAll, describe, it, expect } from 'bun:test';
import React, { act, useEffect } from 'react';
import { Window } from 'happy-dom';
import type { Root } from 'react-dom/client';
import { getBackend, resetBackend } from '@/lib/backend';
import { AuthProvider, useAuth } from '@/features/auth/context/AuthContext';
import { EntriesProvider, useEntries, needsBreachRecheck, passwordEntryToPreview } from '@/features/entries/context/EntriesContext';
import { SettingsProvider } from '@/features/settings/context/SettingsContext';
import { ToastProvider } from '@/contexts/ToastContext';

const dom = new Window();
const previous = new Map<string, PropertyDescriptor | undefined>();
let root: Root | undefined;
let mount: typeof import('react-dom/client').createRoot;
let auth!: ReturnType<typeof useAuth>;
let entries!: ReturnType<typeof useEntries>;
const fixture = { id:'audit-entry', title:'Synthetic audit fixture', username:'fixture-user', password:'ONLY_SYNTHETIC_AUDIT_SECRET', url:'', email:'', notes:'', tags:[], favorite:false, pinned:false, customFields:[], custom_fields:[], attachments:[], createdAt:'2026-09-27', updatedAt:'2026-09-27', created_at:'2026-09-27', updated_at:'2026-09-27', breach_status:{type:'Unknown'} };
beforeAll(async () => {
  for (const [key,value] of Object.entries({window:dom, document:dom.document, HTMLElement:dom.HTMLElement, Element:dom.Element, SVGElement:dom.SVGElement, navigator:dom.navigator, localStorage:dom.localStorage, CustomEvent:dom.CustomEvent, IS_REACT_ACT_ENVIRONMENT:true})) {
    previous.set(key,Object.getOwnPropertyDescriptor(globalThis,key));
    Object.defineProperty(globalThis,key,{configurable:true,writable:true,value});
  }
  mount=(await import('react-dom/client')).createRoot;
});
afterEach(async () => { await act(async()=>root?.unmount()); root=undefined; resetBackend(); dom.document.body.innerHTML=''; dom.localStorage.clear(); });
afterAll(()=> { for(const [key,descriptor] of previous) { if(descriptor) Object.defineProperty(globalThis,key,descriptor); else Reflect.deleteProperty(globalThis,key); } dom.happyDOM.abort(); });
async function setup(overrides: Record<string,unknown>={}) {
  Object.assign(dom,{__TAURI_EVENT_PLUGIN_INTERNALS__:{unregisterListener:()=>{}},__TAURI_INTERNALS__:{transformCallback:()=>1,unregisterCallback:()=>{},invoke:async(command:string)=>{
    if(command==='list_entries')return [{...fixture}]; if(command==='get_entry')return {...fixture}; if(command==='get_tags')return []; return null;
  }}});
  const backend=await getBackend(); Object.assign(backend,overrides);
  dom.localStorage.setItem('yntra-vault-settings',JSON.stringify({disableSkeletonDelays:true}));
  function Harness(){const a=useAuth(),e=useEntries();useEffect(()=>{auth=a;entries=e;},[a,e]);return null;}
  const container=dom.document.createElement('div');dom.document.body.append(container);
  await act(async()=>{root=mount(container as unknown as Element);root.render(<ToastProvider><SettingsProvider><AuthProvider><EntriesProvider><Harness/></EntriesProvider></AuthProvider></SettingsProvider></ToastProvider>);});
  await act(async()=>{auth.setCurrentVault({id:'synthetic-vault',name:'Synthetic',path:'/does-not-exist-audit.vdb'});auth.setIsLocked(false);});
  return backend;
}
describe('session mutation security regressions',()=>{
  it('discards an old add response after switching to another unlocked vault',async()=>{
    let resolve!: (id:string)=>void;
    await setup({addEntry:async()=>new Promise<string>(r=>{resolve=r;})});
    let pending!:Promise<void>;
    await act(async()=>{pending=entries.addEntry(fixture as any);});
    await act(async()=>{auth.setIsLocked(true);auth.setCurrentVault({id:'other',name:'Other',path:'/other.vdb'});auth.setIsLocked(false);resolve('old-session-entry');await pending;});
    expect(entries.entries.some(entry=>entry.id==='old-session-entry')).toBe(false);
    expect(entries.selectedEntry).toBeNull();
  });
  it('discards an add response received after vault lock',async()=>{
    let resolve!: (id:string)=>void;
    await setup({addEntry:async()=>new Promise<string>(r=>{resolve=r;})});
    let pending!:Promise<void>;
    await act(async()=>{pending=entries.addEntry(fixture as any);});
    await act(async()=>{await auth.lockVault();});
    expect(auth.isLocked).toBe(true);expect(entries.entries).toEqual([]);expect(entries.selectedEntry).toBeNull();
    await act(async()=>{resolve('new-audit-entry');await pending;});
    expect(auth.isLocked).toBe(true);
    expect(entries.selectedEntry).toBeNull();
    expect(entries.entries).toEqual([]);
  });
  it('discards a rollback received after vault lock',async()=>{
    let reject!: (error:Error)=>void;
    await setup({updateEntry:async()=>new Promise<void>((_,r)=>{reject=r;})});
    await act(async()=>{await entries.selectEntryById(fixture.id);});
    let pending!:Promise<void>;
    await act(async()=>{pending=entries.updateEntry({...entries.selectedEntry!,title:'Updated fixture'});});
    await act(async()=>{await auth.lockVault();});
    expect(entries.selectedEntry).toBeNull();
    await act(async()=>{reject(new Error('Synthetic IPC failure after lock'));await pending;});
    expect(auth.isLocked).toBe(true);expect(entries.selectedEntry).toBeNull();
  });
  it('stores only list previews after saving and deselection',async()=>{
    await setup({updateEntry:async()=>{}});
    await act(async()=>{await entries.selectEntryById(fixture.id);});
    await act(async()=>{await entries.updateEntry({...entries.selectedEntry!,title:'Updated fixture'});});
    await act(async()=>{await entries.selectEntryById(null);});
    expect(entries.selectedEntry).toBeNull();expect(entries.entries[0]?.password).toBe('••••••••');
  });
  it('never projects recovery, TOTP, notes, custom secrets or attachments into preview state',()=>{
    const preview=passwordEntryToPreview({...fixture,notes:'private-note',totpSecret:'private-totp',recoveryCodes:'private-recovery',customFields:[{id:'s',name:'Secret',type:'password',value:'private-field'}],newAttachments:[{name:'secret.txt',mime_type:'text/plain',data:[1,2,3]}]} as any);
    expect(preview.password).toBe('••••••••');expect(preview.totpSecret).toBe('has-totp');
    expect(preview.notes).toBe('');expect(preview.customFields).toEqual([]);
    expect(preview.recoveryCodes).toBeUndefined();expect(preview.newAttachments).toBeUndefined();
  });
  it('rechecks old known breach results without repeatedly querying fresh ones',()=>{
    const now=Date.parse('2026-09-27T12:00:00Z');
    expect(needsBreachRecheck({type:'Unknown'},now)).toBe(true);
    expect(needsBreachRecheck({type:'Safe',checked_at:'2026-09-26T11:00:00Z'},now)).toBe(true);
    expect(needsBreachRecheck({type:'Breached',checked_at:'2026-09-27T11:00:00Z',breach_count:1},now)).toBe(false);
    expect(needsBreachRecheck({type:'Safe',checked_at:'bad timestamp'},now)).toBe(true);
  });
});

describe('network privacy regressions',()=>{
  it('removes the legacy domain cache and forgets in-memory icons when locked',async()=>{
    const { Favicon, clearFaviconCache }=await import('@/features/entries/components/Favicon');
    await act(async()=>clearFaviconCache());
    let requests=0;
    Object.assign(dom,{__TAURI_EVENT_PLUGIN_INTERNALS__:{unregisterListener:()=>{}},__TAURI_INTERNALS__:{transformCallback:()=>1,unregisterCallback:()=>{},invoke:async(command:string)=>{
      if(command==='get_favicon'){requests++;return 'data:image/png;base64,synthetic-fixture';}return null;
    }}});
    await getBackend();
    dom.localStorage.setItem('yntra-favicons-cache',JSON.stringify({'legacy.test':'data:image/png;base64,legacy'}));
    function Harness(){const a=useAuth();useEffect(()=>{auth=a;},[a]);return !a.isLocked?<Favicon title="Synthetic" url="https://synthetic-sensitive-service.test"/>:null;}
    const container=dom.document.createElement('div');dom.document.body.append(container);
    await act(async()=>{root=mount(container as unknown as Element);root.render(<ToastProvider><SettingsProvider><AuthProvider><Harness/></AuthProvider></SettingsProvider></ToastProvider>);});
    await act(async()=>auth.setIsLocked(false));
    expect(requests).toBe(1);
    await act(async()=>auth.lockVault());
    expect(dom.localStorage.getItem('yntra-favicons-cache')).toBeNull();
    await act(async()=>auth.setIsLocked(false));
    expect(requests).toBe(2);
    await act(async()=>clearFaviconCache());
  });
  it('airgap mode blocks icon requests even with the icon preference enabled',async()=>{
    const { Favicon, clearFaviconCache } = await import('@/features/entries/components/Favicon');
    const { useSettings } = await import('@/features/settings/context/SettingsContext');
    let requests=0;
    Object.assign(dom,{__TAURI_EVENT_PLUGIN_INTERNALS__:{unregisterListener:()=>{}},__TAURI_INTERNALS__:{transformCallback:()=>1,unregisterCallback:()=>{},invoke:async(command:string)=>{
      if(command==='get_favicon'){requests++;return 'data:image/png;base64,synthetic-fixture';}
      return null;
    }}});
    await getBackend();
    dom.localStorage.setItem('yntra-vault-settings',JSON.stringify({operationMode:'airgap',externalFaviconsEnabled:true}));
    let currentSettings:any;
    function Harness(){const a=useAuth(),s=useSettings();useEffect(()=>{auth=a;currentSettings=s;},[a,s]);return !a.isLocked?<Favicon title="Synthetic" url="https://synthetic-sensitive-service.test"/>:null;}
    const container=dom.document.createElement('div');dom.document.body.append(container);
    await act(async()=>{root=mount(container as unknown as Element);root.render(<ToastProvider><SettingsProvider><AuthProvider><Harness/></AuthProvider></SettingsProvider></ToastProvider>);});
    await act(async()=>auth.setIsLocked(false));
    expect(currentSettings.settings.operationMode).toBe('airgap');
    expect(requests).toBe(0);
    await act(async()=>{await new Promise(resolve=>setTimeout(resolve,1100));});
    await act(async()=>{await auth.lockVault();});
    expect(auth.isLocked).toBe(true);
    expect(dom.localStorage.getItem('yntra-favicons-cache')).toBeNull();
    await act(async()=>clearFaviconCache());
  });
});

