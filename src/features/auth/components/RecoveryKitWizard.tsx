import { useEffect, useId, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { ArrowLeft, ArrowRight, Check, Download, Eye, EyeOff, KeyRound, Loader2, ShieldCheck } from 'lucide-react';
import { getBackend, type EmergencyKit } from '@/lib/backend';
import { useTranslation } from '@/contexts/LanguageContext';

const secondary = 'inline-flex items-center justify-center gap-2 h-9 rounded-[3px] border border-[var(--border)] px-3 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] disabled:opacity-50';
const primary = 'inline-flex items-center justify-center gap-2 h-9 rounded-[3px] bg-[var(--text-primary)] px-4 text-[12px] font-medium text-[var(--bg-base)] hover:opacity-90 disabled:opacity-40';

export function RecoveryKitCards({ kit, onDone }: { kit: EmergencyKit; onDone?: () => void }) {
  const { language } = useTranslation();
  const c = (sv: string, en: string) => language === 'sv' ? sv : en;
  const [step, setStep] = useState(0);
  const [shareIndex, setShareIndex] = useState(0);
  const [saved, setSaved] = useState<number[]>([]);
  const [visible, setVisible] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [exported, setExported] = useState<number[]>([]);
  const share = kit.shares[shareIndex];
  const savedCount = new Set(saved).size;
  const titles = [c('Förbered', 'Prepare'), c('Spara delar', 'Save shares'), c('Kontrollera', 'Review')];
  const chooseShare = (index: number) => { setShareIndex(index); setVisible(false); setError(''); };
  const exportShare = async () => {
    if (!share || busy) return;
    setError(''); setBusy(true);
    try {
      const backend = await getBackend();
      const path = await backend.saveFileDialog({ defaultPath: `yntra-recovery-${kit.verification_hash}-share-${share.share_index}.txt`, filters: [{ name: 'Recovery share', extensions: ['txt'] }] });
      if (path) {
        await backend.exportRecoveryShare(path, share.share_data);
        setExported(values => [...new Set([...values, share.share_index])]);
      }
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  };

  return <div className="w-full min-w-0 overflow-hidden rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-primary)] shadow-xl">
    <header className="flex items-center gap-2.5 border-b border-[var(--border-subtle)] bg-[var(--bg-base)] px-5 py-3.5">
      <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)]"><KeyRound size={14}/></div>
      <div className="min-w-0"><h2 className="text-[14px] font-medium">{c('Säkra din återställning', 'Secure your recovery')}</h2><p className="truncate text-[11px] text-[var(--text-tertiary)]">{kit.vault_name} · {c('Två av tre delar', 'Two of three shares')}</p></div>
    </header>
    <ol aria-label={c('Framsteg', 'Progress')} className="flex gap-2 border-b border-[var(--border-subtle)] px-5 pb-2 pt-3">
      {titles.map((title,index)=><li key={title} aria-current={step===index?'step':undefined} className="flex min-w-0 flex-1 flex-col items-center gap-1.5"><div className={`h-1 w-full rounded-full ${index<=step?'bg-[var(--text-primary)]':'bg-[var(--border-subtle)]'}`}/><span className={`text-[10px] font-medium ${index===step?'text-[var(--text-primary)]':'text-[var(--text-tertiary)]'}`}>{title}</span></li>)}
    </ol>
    <div className="flex min-h-[260px] flex-col gap-4 p-5 text-[12px] leading-relaxed">
      {step===0 && <>
        <div className="flex h-11 w-11 items-center justify-center rounded-full border border-[var(--border)] bg-[var(--bg-base)]"><ShieldCheck size={20}/></div>
        <div><h3 className="mb-1 text-[14px] font-medium">{c('En reservväg till ditt valv', 'A backup way into your vault')}</h3><p className="text-[var(--text-secondary)]">{c('Två olika delar återställer åtkomsten om du glömmer lösenordet eller förlorar USB-stickan.', 'Two different shares restore access if you forget your password or lose your USB drive.')}</p></div>
        <div className="rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] p-3 text-[var(--text-secondary)]">{c('Spara delarna på separata säkra platser, inte tillsammans med valvet. Behåll även en säkerhetskopia av själva valvfilen.', 'Keep shares in separate safe places, away from the vault. Also keep a backup of the vault file itself.')}</div>
        <p className="text-[11px] text-[var(--text-tertiary)]">{c('Detta kit gäller den här valvkopian. Äldre kit kan fortfarande öppna äldre säkerhetskopior.', 'This kit belongs to this vault copy. Previous kits may still open older backups.')}</p>
      </>}
      {step===1 && share && <>
        <div role="group" aria-label={c('Välj recovery-del', 'Choose recovery share')} className="flex gap-2">
          {kit.shares.map((item,index)=><button type="button" key={item.share_index} disabled={busy} aria-pressed={index===shareIndex} onClick={()=>chooseShare(index)} className={`flex flex-1 items-center justify-center gap-1.5 rounded-[3px] border py-2 text-[12px] ${index===shareIndex?'border-[var(--text-primary)] bg-[var(--bg-base)]':'border-[var(--border)] text-[var(--text-tertiary)]'}`}>
            {saved.includes(item.share_index)&&<Check size={12}/>} {c('Del', 'Share')} {item.share_index}
          </button>)}
        </div>
        <div className="rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] p-4">
          <div className="mb-3 flex items-center justify-between"><h3 className="font-medium">{c('Recovery-del', 'Recovery share')} {share.share_index}</h3><span className="text-[10px] text-[var(--text-tertiary)]">{shareIndex+1} / {kit.shares.length}</span></div>
          <button type="button" className={`${primary} w-full`} disabled={busy} onClick={exportShare}>{busy?<Loader2 size={14} className="animate-spin"/>:<Download size={14}/>} {c('Spara den här delen', 'Save this share')}</button>
          {exported.includes(share.share_index)&&<p role="status" className="mt-2 text-[11px] text-[var(--text-secondary)]">{c('Filen är sparad. Förvara den åtskild från de andra delarna.', 'File saved. Keep it separate from the other shares.')}</p>}
          <button type="button" className="mt-3 inline-flex items-center gap-1.5 text-[11px] text-[var(--text-tertiary)] hover:text-[var(--text-primary)]" aria-expanded={visible} onClick={()=>setVisible(value=>!value)}>{visible?<EyeOff size={13}/>:<Eye size={13}/>} {visible?c('Dölj delen', 'Hide share'):c('Visa för avskrift', 'Show for transcription')}</button>
          {visible&&<code className="mt-2 block max-h-32 overflow-y-auto whitespace-pre-wrap break-all rounded-[3px] border border-[var(--border)] p-2 text-[11px] select-all">{share.share_data}</code>}
        </div>
        <label className="flex cursor-pointer items-start gap-2.5 text-[var(--text-secondary)]"><input className="mt-1 accent-[var(--text-primary)]" type="checkbox" checked={saved.includes(share.share_index)} onChange={event=>setSaved(values=>event.target.checked?[...new Set([...values,share.share_index])]:values.filter(index=>index!==share.share_index))}/>{c('Jag har sparat den här delen på en säker plats.', 'I have stored this share in a safe place.')}</label>
        <p aria-live="polite" className="text-[11px] text-[var(--text-tertiary)]">{savedCount} / {kit.shares.length} {c('delar bekräftade · minst två krävs', 'shares confirmed · at least two required')}</p>
      </>}
      {step===2 && <>
        <div className="flex h-11 w-11 items-center justify-center rounded-full border border-[var(--border)] bg-[var(--bg-base)]"><ShieldCheck size={20}/></div>
        <div><h3 className="mb-1 text-[14px] font-medium">{c('Redo om något händer', 'Ready when you need it')}</h3><p className="text-[var(--text-secondary)]">{c('Du har bekräftat att minst två delar är sparade. Behåll dem åtskilda och säkra.', 'You confirmed that at least two shares are stored. Keep them separate and secure.')}</p></div>
        <div className="divide-y divide-[var(--border-subtle)] rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)]">{kit.shares.map(item=><div key={item.share_index} className="flex items-center justify-between px-3 py-2"><span>{c('Del','Share')} {item.share_index}</span><span className="text-[11px] text-[var(--text-tertiary)]">{saved.includes(item.share_index)?c('Bekräftad','Confirmed'):c('Inte bekräftad','Not confirmed')}</span></div>)}</div>
        <p className="text-[11px] text-[var(--text-tertiary)]">{c('Du behöver också valvfilen vid återställning. Det går inte att visa delarna igen efter att du stänger guiden.', 'Recovery also requires the vault file. These shares cannot be shown again after closing the guide.')}</p>
      </>}
      {error&&<p role="alert" className="break-words text-[var(--destructive)]">{error}</p>}
    </div>
    <footer className="flex items-center justify-between gap-2 border-t border-[var(--border-subtle)] bg-[var(--bg-base)] px-5 py-3">
      {step>0?<button type="button" className={secondary} disabled={busy} onClick={()=>{setVisible(false);setStep(step-1);}}><ArrowLeft size={13}/>{c('Tillbaka','Back')}</button>:<span className="text-[10px] text-[var(--text-tertiary)]">{c('Förvaras separat','Store separately')}</span>}
      {step===0?<button type="button" className={primary} onClick={()=>setStep(1)}>{c('Börja spara','Start saving')}<ArrowRight size={13}/></button>:step===1?<div className="flex gap-2">{shareIndex<kit.shares.length-1&&<button type="button" className={secondary} disabled={busy} onClick={()=>chooseShare(shareIndex+1)}>{c('Nästa del','Next share')}</button>}<button type="button" className={primary} disabled={busy||savedCount<2} onClick={()=>{setVisible(false);setStep(2);}}>{c('Fortsätt','Continue')}<ArrowRight size={13}/></button></div>:<button type="button" className={primary} disabled={savedCount<2} onClick={onDone}><Check size={13}/>{c('Klart','Done')}</button>}
    </footer>
  </div>;
}

export function RecoveryKitDialog({ kit, onDone }: { kit: EmergencyKit; onDone: () => void }) {
  const dialog = useRef<HTMLDivElement>(null);
  const id = useId();
  const { language } = useTranslation();
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    dialog.current?.focus();
    const trap = (event: KeyboardEvent) => {
      if(event.key==='Escape') { event.preventDefault();event.stopImmediatePropagation(); }
      if(event.key!=='Tab')return;
      const focusable=dialog.current?.querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled)');
      if(!focusable?.length)return;
      const first=focusable[0];const last=focusable[focusable.length-1];
      if(event.shiftKey&&(document.activeElement===first||document.activeElement===dialog.current)){event.preventDefault();last.focus();}
      else if(!event.shiftKey&&document.activeElement===last){event.preventDefault();first.focus();}
    };
    document.addEventListener('keydown',trap,true);
    return ()=>{document.removeEventListener('keydown',trap,true);previous?.focus();};
  },[]);
  return createPortal(<div className="fixed inset-0 z-[100] flex items-start justify-center overflow-y-auto bg-black/50 p-3 sm:items-center sm:p-4">
    <div ref={dialog} tabIndex={-1} role="dialog" aria-modal="true" aria-labelledby={id} className="my-auto w-full max-w-[420px] outline-none"><span id={id} className="sr-only">{language==='sv'?'Spara recovery-delar':'Save recovery shares'}</span><RecoveryKitCards key={kit.verification_hash} kit={kit} onDone={onDone}/></div>
  </div>,document.body);
}
