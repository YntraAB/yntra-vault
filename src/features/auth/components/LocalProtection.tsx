import { useCapabilities } from '@/lib/platform';
import { RecoveryKitDialog } from './RecoveryKitWizard';
export { RecoveryKitCards } from './RecoveryKitWizard';
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { KeyRound, Usb, RefreshCw, Loader2, FileKey, X } from 'lucide-react';
import { getBackend, openFileDialog, type EmergencyKit, type VaultInfo } from '@/lib/backend';
import { useTranslation } from '@/contexts/LanguageContext';
import { SecureSecretInput } from '@/components/ui';
import { SettingRow, SettingSection } from '@/features/settings/components/SettingSection';
import { ProtectionDialog, ProtectionPanel, protectionButton, protectionPrimary } from './ProtectionDialog';
import { isValidNewMasterPassword } from '@/lib/masterPassword';

export interface UsbStorageDevice { id: string; name: string; }
export interface LocalProtectionInfo { protected: boolean; usb_bound: boolean; recovery_enabled: boolean; }
const field = 'h-8 w-full min-w-0 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 text-[12px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)] disabled:opacity-50';
const button = protectionButton;
function useCopy() {
  const { language } = useTranslation();
  return (sv: string, en: string) => language === 'sv' ? sv : en;
}

export function UsbPicker({ value, onChange, disabled = false }: { value: string; onChange: (id: string) => void; disabled?: boolean }) {
  const [devices, setDevices] = useState<UsbStorageDevice[]>([]);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(true);
  const selection = useRef({ value, onChange });
  useLayoutEffect(() => { selection.current = { value, onChange }; }, [value, onChange]);
  const sequence = useRef(0);
  const mounted = useRef(false);
  const c = useCopy();
  const refresh = async () => {
    const request = ++sequence.current;
    setBusy(true); setError('');
    try {
      const devices = await (await getBackend()).listUsbStorageDevices();
      if (!mounted.current || request !== sequence.current) return;
      setDevices(devices);
      if (selection.current.value && !devices.some(device => device.id === selection.current.value)) selection.current.onChange('');
    } catch (e) {
      if (!mounted.current || request !== sequence.current) return;
      setDevices([]); selection.current.onChange(''); setError(String(e));
    } finally { if (mounted.current && request === sequence.current) setBusy(false); }
  };
  useEffect(() => { mounted.current = true; void refresh(); return () => { mounted.current = false; }; }, []);
  return <div className="flex min-w-0 flex-col gap-2">
    <div className="flex items-center gap-2">
      <select aria-label={c('USB-sticka', 'USB drive')} className={field + ' flex-1'} value={value} disabled={disabled || busy} onChange={event => onChange(event.target.value)}>
        <option value="">{busy ? c('Söker efter USB…', 'Finding drives…') : c('Välj USB-sticka', 'Select a USB drive')}</option>
        {devices.map(device => <option key={device.id} value={device.id}>{device.name}{devices.filter(other => other.name === device.name).length > 1 ? ' · ' + device.id.slice(0, 8) : ''}</option>)}
      </select>
      <button type="button" className={button + ' !px-2'} disabled={busy || disabled} onClick={refresh} aria-label={c('Sök efter USB-stickor', 'Refresh USB drives')} title={c('Sök igen', 'Refresh')}><RefreshCw size={13} className={busy ? 'animate-spin' : ''}/></button>
    </div>
    {!busy && !error && !devices.length && <p className="text-[11px] text-[var(--text-tertiary)]">{c('Anslut en USB-sticka och sök igen.', 'Connect a USB drive and refresh.')}</p>}
    {error && <p role="alert" className="break-words text-[11px] text-[var(--destructive)]">{error}</p>}
  </div>;
}

type ProtectionAction = 'generate' | 'bind' | 'unbind' | 'revoke';
export function LocalProtectionSettings({ onProtectionChanged, hardwareActive = false }: {
  onProtectionChanged?: (info: LocalProtectionInfo) => void; hardwareActive?: boolean;
} = {}) {
  const c = useCopy();
  const capabilities = useCapabilities();
  const [info, setInfo] = useState<LocalProtectionInfo | null>(null);
  const [loadError, setLoadError] = useState('');
  const [loading, setLoading] = useState(true);
  const [panel, setPanel] = useState<'usb' | 'recovery' | null>(null);
  const [password, setPassword] = useState('');
  const [keyfile, setKeyfile] = useState('');
  const [usb, setUsb] = useState('');
  const [kit, setKit] = useState<EmergencyKit | null>(null);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const pending = useRef(false);
  const panelVersion = useRef(0);
  const alive = useRef(true);
  const changed = useRef(onProtectionChanged);
  useLayoutEffect(() => { changed.current = onProtectionChanged; }, [onProtectionChanged]);
  const refresh = async () => {
    setLoading(true); setLoadError('');
    try {
      const current = await (await getBackend()).getLocalProtection();
      if (!alive.current) return;
      setInfo(current); changed.current?.(current);
    } catch (e) { if (alive.current) setLoadError(String(e)); }
    finally { if (alive.current) setLoading(false); }
  };
  useEffect(() => { alive.current = true; void refresh(); return () => { alive.current = false; }; }, []);
  const close = () => {
    if (pending.current) return;
    panelVersion.current++;
    setPanel(null); setPassword(''); setKeyfile(''); setUsb(''); setError('');
  };
  const open = (next: 'usb' | 'recovery') => { panelVersion.current++; setError(''); setPassword(''); setKeyfile(''); setPanel(next); };
  const run = async (action: ProtectionAction) => {
    if (pending.current || !password) return;
    pending.current = true; setBusy(true); setError('');
    try {
      const backend = await getBackend();
      if (!alive.current) return;
      if (action === 'generate') {
        const nextKit = await backend.generateEmergencyKit(password, keyfile || undefined);
        if (alive.current) setKit(nextKit);
      } else if (action === 'revoke') await backend.revokeRecovery(password, keyfile || undefined);
      else await backend.setUsbBinding(password, keyfile || undefined, action === 'bind' ? usb : undefined);
      if (!alive.current) return;
      setPassword(''); setKeyfile('');
      if (action !== 'generate') { setPanel(null); setUsb(''); }
      await refresh();
    } catch (e) { if (alive.current) setError(String(e)); }
    finally { pending.current = false; if (alive.current) setBusy(false); }
  };
  const selectKeyfile = async () => {
    const version = panelVersion.current;
    try {
      const selected = await openFileDialog({ multiple: false });
      if (alive.current && version === panelVersion.current && typeof selected === 'string') setKeyfile(selected);
    } catch (e) { if (alive.current && version === panelVersion.current) setError(String(e)); }
  };
  const needsKit = panel === 'usb' && !info?.recovery_enabled;
  const generating = panel === 'recovery' || needsKit;
  const title = panel === 'usb' ? c('USB-skydd', 'USB protection') : c('Återställningsnycklar', 'Recovery keys');
  const unavailable = loading || !!loadError || !info;
  const authFields = <div className="flex flex-col gap-2 border-t border-[var(--border-subtle)] pt-4">
    <label className="flex flex-col gap-1.5 text-[12px] text-[var(--text-secondary)]">{c('Huvudlösenord', 'Master password')}<SecureSecretInput value={password} onChange={setPassword} placeholder={c('Nuvarande lösenord', 'Current password')} disabled={busy}/></label>
    <div className="flex items-center gap-1">
      <button type="button" className="inline-flex items-center gap-1.5 py-1 text-[11px] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] disabled:opacity-50" disabled={busy} onClick={selectKeyfile}><FileKey size={12}/>{keyfile ? c('Nyckelfil vald', 'Key file selected') : c('Använd nyckelfil', 'Use a key file')}</button>
      {keyfile && <button type="button" disabled={busy} onClick={() => setKeyfile('')} aria-label={c('Ta bort vald nyckelfil', 'Clear selected key file')} className="p-1 text-[var(--text-tertiary)]"><X size={12}/></button>}
    </div>
  </div>;
  return <>
    <SettingSection label={c('Åtkomst och återställning', 'Access and recovery')}>
      {capabilities.usbBinding && <SettingRow label={c('USB-skydd', 'USB protection')} description={loading ? c('Läser status…', 'Loading status…') : loadError ? c('Status kunde inte läsas.', 'Status unavailable.') : info?.usb_bound ? c('Aktivt · USB-stickan krävs vid upplåsning.', 'Active · your USB drive is required to unlock.') : c('Kräv en USB-sticka tillsammans med lösenordet.', 'Require a USB drive alongside your password.')}>
        <button type="button" className={button} disabled={unavailable || hardwareActive} onClick={() => open('usb')}>{info?.usb_bound ? c('Hantera', 'Manage') : c('Ställ in', 'Set up')}</button>
      </SettingRow>}
      <SettingRow label={c('Återställningsnycklar', 'Recovery keys')} description={loading ? c('Läser status…', 'Loading status…') : loadError ? c('Status kunde inte läsas.', 'Status unavailable.') : info?.recovery_enabled ? c('Aktiva · två av tre nycklar återställer åtkomsten.', 'Active · two of three shares restore access.') : c('En reservväg om du förlorar åtkomsten.', 'A backup way to regain access.')}>
        <button type="button" className={button} disabled={unavailable || hardwareActive} onClick={() => open('recovery')}>{info?.recovery_enabled ? c('Hantera', 'Manage') : c('Skapa', 'Create')}</button>
      </SettingRow>
      {hardwareActive && <p className="mt-2 text-[11px] text-[var(--text-tertiary)]">{c('Stäng av säkerhetsnyckeln ovan för att använda USB-skydd eller återställningsnycklar.', 'Disable the security key above to use USB protection or recovery keys.')}</p>}
      {loadError && <div className="mt-2 flex items-start gap-2"><p role="alert" className="min-w-0 flex-1 break-words text-[11px] text-[var(--destructive)]">{loadError}</p><button type="button" className={button} disabled={loading} onClick={refresh}>{c('Försök igen', 'Retry')}</button></div>}
    </SettingSection>
    {panel && !kit && <ProtectionDialog label={title} onClose={busy ? undefined : close}>
      <ProtectionPanel title={title} subtitle={c('Gäller det här valvet', 'For this vault')} icon={panel === 'usb' ? <Usb size={14}/> : <KeyRound size={14}/>} onClose={busy ? undefined : close} footer={<>
        <button type="button" className={button} disabled={busy} onClick={close}>{c('Avbryt', 'Cancel')}</button>
        <button type="button" className={protectionPrimary} disabled={busy || !password || unavailable || (!generating && !usb)} onClick={() => run(generating ? 'generate' : 'bind')}>{busy && <Loader2 size={13} className="animate-spin"/>}{generating ? info?.recovery_enabled ? c('Skapa nya nycklar', 'Replace keys') : c('Skapa nycklar', 'Create keys') : c('Aktivera USB-skydd', 'Enable USB protection')}</button>
      </>}>
        <div className="flex flex-col gap-4">
          <p className="text-[var(--text-secondary)]">{needsKit ? c('Spara återställningsnycklar först, så att du kan öppna valvet om stickan försvinner.', 'Save recovery keys first, so you can open the vault if the drive is lost.') : panel === 'usb' ? c('Välj den USB-sticka som ska krävas vid upplåsning.', 'Choose the USB drive required to unlock this vault.') : info?.recovery_enabled ? c('Nya nycklar ersätter de gamla för den här valvfilen. Äldre säkerhetskopior behåller sina nycklar.', 'New keys replace the old ones for this vault file. Older backups keep their keys.') : c('Spara tre nycklar separat. Två av dem och valvfilen räcker för att återställa åtkomsten.', 'Store three shares separately. Any two and the vault file can restore access.')}</p>
          {panel === 'usb' && !needsKit && <UsbPicker value={usb} onChange={setUsb} disabled={busy}/>}
          {!info?.protected && <p className="border-l-2 border-[var(--border)] pl-3 text-[11px] text-[var(--text-secondary)]">{c('Aktiveringen tar bort biometrisk upplåsning. Länkade enheter behöver paras om.', 'Enabling this removes biometric unlock. Linked devices need to be paired again.')}</p>}
          {authFields}
          {error && <p role="alert" className="break-words text-[var(--destructive)]">{error}</p>}
          {loadError && <p role="alert" className="break-words text-[var(--destructive)]">{loadError}</p>}
          {panel === 'usb' && info?.usb_bound && <button type="button" className={button + ' self-start'} disabled={busy || !password} onClick={() => run('unbind')}>{c('Stäng av USB-skydd', 'Disable USB protection')}</button>}
          {panel === 'recovery' && info?.recovery_enabled && <div className="border-t border-[var(--border-subtle)] pt-3"><button type="button" className={button} disabled={busy || !password || info.usb_bound} onClick={() => run('revoke')}>{c('Återkalla nycklar', 'Revoke keys')}</button><p className="mt-2 text-[11px] text-[var(--text-tertiary)]">{info.usb_bound ? c('Stäng av USB-skyddet innan du återkallar nycklarna.', 'Disable USB protection before revoking the keys.') : c('Återkallade nycklar kan inte återställa den här valvfilen.', 'Revoked keys cannot recover this vault file.')}</p></div>}
          {panel === 'usb' && <details className="text-[11px] text-[var(--text-tertiary)]"><summary className="cursor-pointer hover:text-[var(--text-primary)]">{c('Så fungerar skyddet', 'How protection works')}</summary><p className="mt-2">{c('Bindningen använder stickans serienummer och påverkas inte av filnamn eller placering. Serienumret kan förfalskas; skyddet ersätter inte en säker dator.', 'Binding uses the drive serial number, regardless of filename or location. Serial numbers can be spoofed; this does not replace a secure computer.')}</p></details>}
        </div>
      </ProtectionPanel>
    </ProtectionDialog>}
    {kit && <RecoveryKitDialog key={kit.verification_hash} kit={kit} onDone={() => { setKit(null); if (panel !== 'usb') close(); }}/>}
  </>;
}

export function RecoveryForm({
  path,
  onRecovered,
  onBack,
}: {
  path: string;
  onRecovered: (info: VaultInfo) => void;
  onBack: () => void;
}) {
  const c = useCopy();
  const [a, setA] = useState("");
  const [b, setB] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const recovering = useRef(false);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  return (
    <form
      className="mt-4 flex flex-col gap-3 text-[12px]"
      onSubmit={async (e) => {
        e.preventDefault();
        if (recovering.current) return;
        setError("");
        if (!a.trim() || !b.trim() || a.trim() === b.trim() || !isValidNewMasterPassword(password) || password !== confirm) {
          setError(
            c(
              "Ange två olika återställningsnycklar och samma nya lösenord två gånger, minst 12 tecken.",
              "Enter two different recovery shares and the same new password twice, at least 12 characters.",
            ),
          );
          return;
        }
        recovering.current = true; setBusy(true);
        try {
          const info = await (
            await getBackend()
          ).recoverVault(path, a, b, password);
          setA("");
          setB("");
          setPassword("");
          setConfirm("");
          onRecovered(info);
        } catch (e) {
          setError(String(e));
        } finally {
          recovering.current = false;
          setBusy(false);
        }
      }}
    >
      <p>
        {c(
          "Ange två delar från samma recovery-kit. Återställningen byter lösenord, tar bort USB-bindningen och förbrukar detta kit på den uppdaterade filen. Skapa sedan ett nytt kit och bind USB igen.",
          "Enter two shares from the same kit. Recovery changes the password, removes USB binding and consumes this kit for the updated file. Then create a new kit and bind your USB again.",
        )}
      </p>
      <input
        className={field}
        type="password"
        autoComplete="off"
        maxLength={2080}
        aria-label={c("Recovery-del 1", "Recovery share 1")}
        placeholder={c("Första recovery-delen", "First recovery share")}
        value={a}
        onChange={(e) => setA(e.target.value)}
        disabled={busy}
      />
      <input
        className={field}
        type="password"
        autoComplete="off"
        maxLength={2080}
        aria-label={c("Recovery-del 2", "Recovery share 2")}
        placeholder={c("Andra recovery-delen", "Second recovery share")}
        value={b}
        onChange={(e) => setB(e.target.value)}
        disabled={busy}
      />
      <SecureSecretInput
        value={password}
        onChange={setPassword}
        placeholder={c("Nytt lösenord", "New password")}
        disabled={busy}
      />
      <SecureSecretInput
        value={confirm}
        onChange={setConfirm}
        placeholder={c("Bekräfta nytt lösenord", "Confirm new password")}
        disabled={busy}
      />
      {error && <p role="alert">{error}</p>}
      <button
        className={button}
        disabled={busy || !a.trim() || !b.trim() || a.trim() === b.trim() || !isValidNewMasterPassword(password) || password !== confirm}
        type="submit"
      >
        {c("Återställ åtkomst", "Restore access")}
      </button>
      <button className={button} type="button" disabled={busy} onClick={onBack}>
        {c("Tillbaka", "Back")}
      </button>
    </form>
  );
}
