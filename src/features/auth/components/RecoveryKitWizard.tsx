import { useRef, useState } from 'react';
import { ArrowLeft, ArrowRight, Check, Download, Eye, EyeOff, KeyRound, Loader2 } from 'lucide-react';
import { getBackend, type EmergencyKit } from '@/lib/backend';
import { useTranslation } from '@/contexts/LanguageContext';
import { ProtectionDialog, ProtectionPanel, protectionButton, protectionPrimary } from './ProtectionDialog';

export function RecoveryKitCards({ kit, onDone }: { kit: EmergencyKit; onDone?: () => void }) {
  const { language } = useTranslation();
  const c = (sv: string, en: string) => language === 'sv' ? sv : en;
  const [step, setStep] = useState(0);
  const [shareIndex, setShareIndex] = useState(0);
  const [saved, setSaved] = useState<number[]>([]);
  const [visible, setVisible] = useState(false);
  const [busy, setBusy] = useState(false);
  const inFlight = useRef(false);
  const [error, setError] = useState('');
  const [exported, setExported] = useState<number[]>([]);
  const share = kit.shares[shareIndex];
  const savedCount = new Set(saved).size;
  const titles = [c('Förbered', 'Prepare'), c('Spara', 'Save'), c('Klart', 'Review')];
  const chooseShare = (index: number) => { setShareIndex(index); setVisible(false); setError(''); };
  const exportShare = async () => {
    if (!share || inFlight.current) return;
    inFlight.current = true; setError(''); setBusy(true);
    try {
      const backend = await getBackend();
      const path = await backend.saveFileDialog({ defaultPath: 'yntra-recovery-' + kit.verification_hash + '-share-' + share.share_index + '.txt', filters: [{ name: 'Recovery share', extensions: ['txt'] }] });
      if (path) {
        await backend.exportRecoveryShare(path, share.share_data);
        setExported(values => [...new Set([...values, share.share_index])]);
      }
    } catch (e) { setError(String(e)); }
    finally { inFlight.current = false; setBusy(false); }
  };
  const footer = <>
    {step > 0 ? <button type="button" className={protectionButton} disabled={busy} onClick={() => { setVisible(false); setStep(step - 1); }}><ArrowLeft size={13}/>{c('Tillbaka', 'Back')}</button> : <span className="text-[11px] text-[var(--text-tertiary)]">{c('2 av 3 nycklar behövs', '2 of 3 shares required')}</span>}
    {step === 0 ? <button type="button" className={protectionPrimary} onClick={() => setStep(1)}>{c('Börja spara', 'Start saving')}<ArrowRight size={13}/></button>
      : step === 1 ? <button type="button" className={protectionPrimary} disabled={busy || savedCount < 2} onClick={() => { setVisible(false); setStep(2); }}>{c('Fortsätt', 'Continue')}<ArrowRight size={13}/></button>
      : <button type="button" className={protectionPrimary} disabled={savedCount < 2 || busy} onClick={onDone}><Check size={13}/>{c('Klart', 'Done')}</button>}
  </>;

  return <ProtectionPanel title={c('Återställningsnycklar', 'Recovery keys')} subtitle={kit.vault_name} icon={<KeyRound size={14}/>} footer={footer}>
    <ol aria-label={c('Framsteg', 'Progress')} className="mb-5 flex gap-2">
      {titles.map((title, index) => <li key={title} aria-current={step === index ? 'step' : undefined} className="flex min-w-0 flex-1 flex-col gap-1.5"><div className={'h-1 rounded-full ' + (index <= step ? 'bg-[var(--text-primary)]' : 'bg-[var(--border-subtle)]')}/><span className="text-[10px] text-[var(--text-tertiary)]">{title}</span></li>)}
    </ol>
    <div className="flex flex-col gap-4">
      {step === 0 && <>
        <p className="text-[var(--text-secondary)]">{c('Två olika nycklar och din valvfil återställer åtkomsten om du förlorar lösenordet eller USB-stickan.', 'Two different shares and your vault file restore access if you lose your password or USB drive.')}</p>
        <div className="divide-y divide-[var(--border-subtle)] border-y border-[var(--border-subtle)] text-[var(--text-secondary)]">
          <p className="py-2.5">{c('Spara nycklarna på separata, säkra platser.', 'Store the shares in separate, safe places.')}</p>
          <p className="py-2.5">{c('Behåll också en säkerhetskopia av valvfilen.', 'Also keep a backup of your vault file.')}</p>
        </div>
        <p className="text-[11px] text-[var(--text-tertiary)]">{c('Nycklarna visas bara nu. Äldre nycklar kan fortfarande öppna äldre säkerhetskopior.', 'These shares are shown only now. Older shares may still open older backups.')}</p>
      </>}
      {step === 1 && share && <>
        <div role="group" aria-label={c('Välj nyckel', 'Choose recovery share')} className="flex gap-2">
          {kit.shares.map((item, index) => <button type="button" key={item.share_index} disabled={busy} aria-pressed={index === shareIndex} onClick={() => chooseShare(index)} className={protectionButton + ' flex-1 ' + (index === shareIndex ? '!border-[var(--border-focus)] !text-[var(--text-primary)]' : '')}>
            {saved.includes(item.share_index) && <Check size={12}/>} {c('Nyckel', 'Share')} {item.share_index}
          </button>)}
        </div>
        <div className="rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] p-3">
          <div className="mb-3 flex items-center gap-2 text-[var(--text-secondary)]"><KeyRound size={14}/><span>{c('Nyckel', 'Share')} {share.share_index}</span></div>
          <button type="button" className={protectionButton + ' w-full'} disabled={busy} onClick={exportShare}>{busy ? <Loader2 size={13} className="animate-spin"/> : <Download size={13}/>} {c('Spara nyckeln', 'Save this share')}</button>
          {exported.includes(share.share_index) && <p role="status" className="mt-2 text-[11px] text-[var(--text-secondary)]">{c('Sparad. Förvara den åtskild från övriga nycklar.', 'File saved. Keep it separate from the other shares.')}</p>}
          <button type="button" className="mt-3 inline-flex items-center gap-1.5 text-[11px] text-[var(--text-tertiary)] hover:text-[var(--text-primary)]" aria-expanded={visible} onClick={() => setVisible(value => !value)}>{visible ? <EyeOff size={13}/> : <Eye size={13}/>} {visible ? c('Dölj nyckeln', 'Hide share') : c('Visa för avskrift', 'Show for transcription')}</button>
          {visible && <code className="mt-2 block max-h-28 overflow-y-auto whitespace-pre-wrap break-all border-t border-[var(--border-subtle)] pt-2 text-[11px] select-all">{share.share_data}</code>}
        </div>
        <label className="flex cursor-pointer items-start gap-2 text-[var(--text-secondary)]"><input className="mt-1 accent-[var(--text-primary)]" type="checkbox" disabled={busy} checked={saved.includes(share.share_index)} onChange={event => setSaved(values => event.target.checked ? [...new Set([...values, share.share_index])] : values.filter(index => index !== share.share_index))}/>{c('Jag har sparat den här nyckeln på en säker plats.', 'I have stored this share in a safe place.')}</label>
        <div className="flex items-center justify-between gap-2 text-[11px] text-[var(--text-tertiary)]"><span aria-live="polite">{savedCount} / 3 {c('sparade · minst 2 krävs', 'confirmed · at least 2 required')}</span>{shareIndex < kit.shares.length - 1 && <button type="button" disabled={busy} onClick={() => chooseShare(shareIndex + 1)} className="inline-flex items-center gap-1 hover:text-[var(--text-primary)]">{c('Nästa nyckel', 'Next share')}<ArrowRight size={12}/></button>}</div>
      </>}
      {step === 2 && <>
        <p className="text-[var(--text-secondary)]">{c('Dina återställningsnycklar är sparade. Förvara dem separat från varandra och valvet.', 'Your recovery shares are saved. Keep them separate from each other and your vault.')}</p>
        <div className="divide-y divide-[var(--border-subtle)] border-y border-[var(--border-subtle)]">{kit.shares.map(item => <div key={item.share_index} className="flex items-center justify-between py-2.5"><span>{c('Nyckel', 'Share')} {item.share_index}</span><span className="flex items-center gap-1.5 text-[11px] text-[var(--text-tertiary)]">{saved.includes(item.share_index) && <Check size={12}/>} {saved.includes(item.share_index) ? c('Bekräftad', 'Confirmed') : c('Inte bekräftad', 'Not confirmed')}</span></div>)}</div>
        <p className="text-[11px] text-[var(--text-tertiary)]">{c('Efter att du stänger går det inte att visa nycklarna igen.', 'These shares cannot be shown again after closing.')}</p>
      </>}
      {error && <p role="alert" className="break-words text-[var(--destructive)]">{error}</p>}
    </div>
  </ProtectionPanel>;
}

export function RecoveryKitDialog({ kit, onDone }: { kit: EmergencyKit; onDone: () => void }) {
  const { language } = useTranslation();
  return <ProtectionDialog label={language === 'sv' ? 'Spara återställningsnycklar' : 'Save recovery shares'}><RecoveryKitCards key={kit.verification_hash} kit={kit} onDone={onDone}/></ProtectionDialog>;
}
