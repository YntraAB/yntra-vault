import { useCapabilities } from '@/lib/platform';
import { RecoveryKitDialog } from './RecoveryKitWizard';
export { RecoveryKitCards } from './RecoveryKitWizard';
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { KeyRound, Usb, RefreshCw, Loader2, FileKey, X, Shield, ArrowRight, ArrowLeft } from 'lucide-react';
import { getBackend, openFileDialog, type EmergencyKit, type VaultInfo } from '@/lib/backend';
import { useTranslation } from '@/contexts/LanguageContext';
import { SecureSecretInput } from '@/components/ui';
import { SettingRow } from '@/features/settings/components/SettingSection';
import {
  ProtectionDialog,
  ProtectionPanel,
  protectionSecondaryButton,
  protectionPrimary,
  protectionDanger,
} from './ProtectionDialog';
import { isValidNewMasterPassword } from '@/lib/masterPassword';

export interface UsbStorageDevice { id: string; name: string; }
export interface LocalProtectionInfo { protected: boolean; usb_bound: boolean; recovery_enabled: boolean; }
const field = 'h-8 w-full min-w-0 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 text-[12px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)] disabled:opacity-50';

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
    setBusy(true);
    setError('');
    try {
      const devices = await (await getBackend()).listUsbStorageDevices();
      if (!mounted.current || request !== sequence.current) return;
      setDevices(devices);
      if (selection.current.value && !devices.some(device => device.id === selection.current.value)) {
        selection.current.onChange(devices.length > 0 ? devices[0].id : '');
      } else if (!selection.current.value && devices.length > 0) {
        selection.current.onChange(devices[0].id);
      }
    } catch (e) {
      if (!mounted.current || request !== sequence.current) return;
      setDevices([]);
      selection.current.onChange('');
      setError(String(e));
    } finally {
      if (mounted.current && request === sequence.current) setBusy(false);
    }
  };

  useEffect(() => {
    mounted.current = true;
    void refresh();
    return () => { mounted.current = false; };
  }, []);

  const effectiveValue = value || (devices.length > 0 ? devices[0].id : '');

  return (
    <div className="flex min-w-0 flex-col gap-1.5">
      <label className="text-[12px] font-medium text-[var(--text-secondary)]">
        {c('USB-sticka', 'USB drive')}
      </label>
      <div className="flex items-center gap-2">
        <select
          aria-label={c('USB-sticka', 'USB drive')}
          className="h-8 flex-1 min-w-0 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 text-[12px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)] transition-colors cursor-pointer disabled:opacity-50"
          value={effectiveValue}
          disabled={disabled || busy}
          onChange={event => onChange(event.target.value)}
        >
          {devices.length === 0 && (
            <option value="">{busy ? c('Söker efter USB…', 'Finding drives…') : c('Ingen USB-sticka hittades', 'No USB drive found')}</option>
          )}
          {devices.map(device => (
            <option key={device.id} value={device.id}>
              {device.name}{devices.filter(other => other.name === device.name).length > 1 ? ' · ' + device.id.slice(0, 8) : ''}
            </option>
          ))}
        </select>
        <button
          type="button"
          className="inline-flex h-8 items-center justify-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[11px] font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer disabled:opacity-50 shrink-0"
          disabled={busy || disabled}
          onClick={refresh}
          aria-label={c('Sök efter USB-stickor', 'Refresh USB drives')}
          title={c('Sök igen', 'Refresh')}
        >
          <RefreshCw size={12} className={busy ? 'animate-spin' : ''} />
          <span>{busy ? c('Söker…', 'Scanning…') : c('Sök igen', 'Refresh')}</span>
        </button>
      </div>
      {!busy && !error && !devices.length && (
        <p className="text-[11px] text-[var(--text-tertiary)]">
          {c('Anslut en USB-sticka och sök igen.', 'Connect a USB drive and refresh.')}
        </p>
      )}
      {error && <p role="alert" className="break-words text-[11px] text-[var(--destructive)]">{error}</p>}
    </div>
  );
}

type ProtectionAction = 'generate' | 'bind' | 'unbind' | 'revoke';
export function LocalProtectionSettings({
  onProtectionChanged,
  hardwareActive = false,
}: {
  onProtectionChanged?: (info: LocalProtectionInfo) => void;
  hardwareActive?: boolean;
} = {}) {
  const c = useCopy();
  const capabilities = useCapabilities();
  const [info, setInfo] = useState<LocalProtectionInfo | null>(null);
  const [loadError, setLoadError] = useState('');
  const [loading, setLoading] = useState(true);
  const [panel, setPanel] = useState<'usb' | 'recovery' | null>(null);
  const [usbStep, setUsbStep] = useState<0 | 1>(0);
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

  useLayoutEffect(() => {
    changed.current = onProtectionChanged;
  }, [onProtectionChanged]);

  const refresh = async () => {
    setLoading(true);
    setLoadError('');
    try {
      const current = await (await getBackend()).getLocalProtection();
      if (!alive.current) return;
      setInfo(current);
      changed.current?.(current);
    } catch (e) {
      if (alive.current) setLoadError(String(e));
    } finally {
      if (alive.current) setLoading(false);
    }
  };

  useEffect(() => {
    alive.current = true;
    void refresh();
    return () => {
      alive.current = false;
    };
  }, []);

  const close = () => {
    if (pending.current) return;
    panelVersion.current++;
    setPanel(null);
    setUsbStep(0);
    setPassword('');
    setKeyfile('');
    setUsb('');
    setError('');
  };

  const cancelKit = () => {
    if (!kit) return;
    const confirmed = typeof window.confirm !== 'function' || window.confirm(c(
      'De nya återställningsnycklarna har inte sparats. Vill du stänga ändå? Du kan skapa ett nytt kit från säkerhetsinställningarna.',
      'The new recovery shares have not been saved. Close anyway? You can create a new kit from the security settings.',
    ));
    if (!confirmed) return;
    setKit(null);
    close();
    void refresh();
  };

  const open = (next: 'usb' | 'recovery') => {
    panelVersion.current++;
    setError('');
    setPassword('');
    setKeyfile('');
    setUsbStep(0);
    setUsb('');
    setPanel(next);
    if (next === 'usb') {
      void getBackend().then(b => b.listUsbStorageDevices()).then(devs => {
        if (devs.length > 0 && alive.current) {
          setUsb(prev => prev || devs[0].id);
        }
      }).catch(() => {});
    }
  };

  const run = async (action: ProtectionAction) => {
    if (pending.current || !password) return;
    pending.current = true;
    setBusy(true);
    setError('');
    try {
      const backend = await getBackend();
      if (!alive.current) return;
      if (action === 'generate') {
        const nextKit = await backend.generateEmergencyKit(password, keyfile || undefined);
        if (alive.current) setKit(nextKit);
      } else if (action === 'revoke') {
        await backend.revokeRecovery(password, keyfile || undefined);
      } else {
        const nextKit = await backend.setUsbBinding(password, keyfile || undefined, action === 'bind' ? usb : undefined);
        if (alive.current) setKit(nextKit);
      }
      if (!alive.current) return;
      setPassword('');
      setKeyfile('');
      if (action !== 'generate') {
        setPanel(null);
        setUsbStep(0);
        setUsb('');
      }
      await refresh();
    } catch (e) {
      if (alive.current) setError(String(e));
    } finally {
      pending.current = false;
      if (alive.current) setBusy(false);
    }
  };

  const selectKeyfile = async () => {
    const version = panelVersion.current;
    try {
      const selected = await openFileDialog({ multiple: false });
      if (alive.current && version === panelVersion.current && typeof selected === 'string') {
        setKeyfile(selected);
      }
    } catch (e) {
      if (alive.current && version === panelVersion.current) setError(String(e));
    }
  };

  const needsKit = panel === 'usb' && !info?.recovery_enabled;
  const generating = panel === 'recovery' || needsKit;
  const title = panel === 'usb' ? c('USB-skydd', 'USB protection') : c('Återställningsnycklar', 'Recovery keys');
  const unavailable = loading || !!loadError || !info;

  const authFields = (
    <div className="flex flex-col gap-2 pt-2 border-t border-[var(--border-subtle)]">
      <label className="text-[12px] font-medium text-[var(--text-secondary)]">
        {c('Huvudlösenord', 'Master password')}
      </label>
      <SecureSecretInput
        value={password}
        onChange={setPassword}
        placeholder={c('Nuvarande lösenord för att godkänna', 'Current password to authorize')}
        disabled={busy}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            if (panel === 'usb' && usbStep === 0 && password.trim()) {
              setUsbStep(1);
            }
          }
        }}
      />
      <div className="flex items-center justify-between text-[11px]">
        <button
          type="button"
          className="inline-flex items-center gap-1.5 py-0.5 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors cursor-pointer disabled:opacity-50"
          disabled={busy}
          onClick={selectKeyfile}
        >
          <FileKey size={12} />
          <span>{keyfile ? c('Nyckelfil vald', 'Key file selected') : c('Använd nyckelfil (valfritt)', 'Use a key file (optional)')}</span>
        </button>
        {keyfile && (
          <button
            type="button"
            disabled={busy}
            onClick={() => setKeyfile('')}
            aria-label={c('Ta bort vald nyckelfil', 'Clear selected key file')}
            className="rounded-[2px] p-0.5 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
          >
            <X size={12} />
          </button>
        )}
      </div>
    </div>
  );

  return (
    <>
      {/* USB Protection Row */}
      {capabilities.usbBinding && (
        <SettingRow
          label={c('USB-skydd', 'USB protection')}
          description={
            loading
              ? c('Läser status…', 'Loading status…')
              : loadError
              ? c('Status kunde inte läsas.', 'Status unavailable.')
              : hardwareActive
              ? c('Stäng av säkerhetsnyckeln ovan för att använda USB-skydd.', 'Disable the security key above to use USB protection.')
              : info?.usb_bound
              ? c('Aktivt · USB-stickan krävs tillsammans med lösenordet vid upplåsning.', 'Active · your USB drive is required alongside your password to unlock.')
              : c('Kräv en ansluten USB-sticka tillsammans med ditt huvudlösenord.', 'Require a physical USB drive alongside your master password.')
          }
          tooltip={c(
            'Binder valvets kryptering till en specifik USB-sticka. Stickan måste vara ansluten vid upplåsning.',
            'Binds the vault encryption to a specific USB drive. The drive must be connected to unlock.'
          )}
        >
          <button
            type="button"
            className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] cursor-pointer whitespace-nowrap shrink-0 disabled:opacity-50 disabled:cursor-not-allowed"
            disabled={unavailable || hardwareActive}
            onClick={() => open('usb')}
          >
            {info?.usb_bound ? c('Hantera', 'Manage') : c('Ställ in', 'Set up')}
          </button>
        </SettingRow>
      )}

      {/* Recovery Keys Row */}
      <SettingRow
        label={c('Återställningsnycklar', 'Recovery keys')}
        description={
          loading
            ? c('Läser status…', 'Loading status…')
            : loadError
            ? c('Status kunde inte läsas.', 'Status unavailable.')
            : hardwareActive
            ? c('Stäng av säkerhetsnyckeln ovan för att använda återställningsnycklar.', 'Disable the security key above to use recovery keys.')
            : info?.recovery_enabled
            ? c('Aktiva · två av tre nycklar återställer åtkomsten.', 'Active · two of three shares restore access.')
            : c('En säker reservväg om du förlorar lösenordet eller USB-stickan.', 'A secure backup way to regain access if you lose your password or USB.')
        }
        tooltip={c(
          'Genererar tre återställningsnycklar baserat på Shamir Secret Sharing. Två nycklar räcker för att återställa valvet.',
          'Generates three recovery shares using Shamir Secret Sharing. Any two shares can restore access.'
        )}
      >
        <button
          type="button"
          className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] cursor-pointer whitespace-nowrap shrink-0 disabled:opacity-50 disabled:cursor-not-allowed"
          disabled={unavailable || hardwareActive}
          onClick={() => open('recovery')}
        >
          {info?.recovery_enabled ? c('Hantera', 'Manage') : c('Skapa', 'Create')}
        </button>
      </SettingRow>

      {/* Error retry indicator */}
      {loadError && (
        <div className="flex items-center justify-between gap-3 border-b border-[var(--border-subtle)] py-2.5">
          <p role="alert" className="min-w-0 flex-1 break-words text-[11px] text-[var(--destructive)]">
            {loadError}
          </p>
          <button
            type="button"
            className="h-7 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[11px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] cursor-pointer shrink-0"
            disabled={loading}
            onClick={refresh}
          >
            {c('Försök igen', 'Retry')}
          </button>
        </div>
      )}

      {/* USB Protection & Recovery Keys Dialog */}
      {panel && !kit && (
        <ProtectionDialog label={title} onClose={busy ? undefined : close}>
          <ProtectionPanel
            title={title}
            subtitle={
              panel === 'usb'
                ? (info?.usb_bound ? c('Aktivt för detta valv', 'Active for this vault') : c('Kräv USB-sticka vid upplåsning', 'Require USB drive to unlock'))
                : (info?.recovery_enabled ? c('Aktiva för detta valv', 'Active for this vault') : c('Reservväg vid förlorad åtkomst', 'Backup access'))
            }
            icon={panel === 'usb' ? <Usb size={14} /> : <KeyRound size={14} />}
            onClose={busy ? undefined : close}
            footer={
              panel === 'usb' ? (
                usbStep === 0 ? (
                  <>
                    <button
                      type="button"
                      className={protectionSecondaryButton}
                      disabled={busy}
                      onClick={close}
                    >
                      {c('Avbryt', 'Cancel')}
                    </button>
                    <button
                      type="button"
                      className={protectionPrimary}
                      disabled={busy || !password || unavailable}
                      onClick={() => setUsbStep(1)}
                    >
                      <span>{info?.usb_bound ? c('Byt USB-sticka', 'Change USB drive') : c('Fortsätt', 'Continue')}</span>
                      <ArrowRight size={13} />
                    </button>
                  </>
                ) : (
                  <>
                    <button
                      type="button"
                      className={protectionSecondaryButton}
                      disabled={busy}
                      onClick={() => setUsbStep(0)}
                    >
                      <ArrowLeft size={13} />
                      <span>{c('Tillbaka', 'Back')}</span>
                    </button>
                    <button
                      type="button"
                      className={protectionPrimary}
                      disabled={busy || !password || !usb}
                      onClick={() => run('bind')}
                    >
                      {busy && <Loader2 size={13} className="animate-spin" />}
                      <span>{info?.usb_bound ? c('Spara ny sticka', 'Save new drive') : c('Aktivera USB-skydd', 'Enable USB protection')}</span>
                    </button>
                  </>
                )
              ) : (
                <>
                  <button
                    type="button"
                    className={protectionSecondaryButton}
                    disabled={busy}
                    onClick={close}
                  >
                    {c('Avbryt', 'Cancel')}
                  </button>
                  <button
                    type="button"
                    className={protectionPrimary}
                    disabled={busy || !password || unavailable}
                    onClick={() => run(generating ? 'generate' : 'bind')}
                  >
                    {busy && <Loader2 size={13} className="animate-spin" />}
                    <span>
                      {info?.recovery_enabled
                        ? c('Skapa nya nycklar', 'Replace keys')
                        : c('Skapa nycklar', 'Create keys')}
                    </span>
                  </button>
                </>
              )
            }
          >
            <AnimatePresence mode="wait">
              {panel === 'usb' && usbStep === 0 && (
                <motion.div
                  key="step-password"
                  initial={{ opacity: 0, y: 6 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -6 }}
                  transition={{ duration: 0.15 }}
                  className="flex flex-col gap-3"
                >
                  <p className="text-[12px] leading-relaxed text-[var(--text-secondary)]">
                    {needsKit
                      ? c('Spara återställningsnycklar först, så att du kan öppna valvet om USB-stickan försvinner.', 'Save recovery keys first, so you can open the vault if the drive is lost.')
                      : info?.usb_bound
                      ? c('Ange ditt huvudlösenord för att byta USB-sticka eller inaktivera skyddet.', 'Enter your master password to change the USB drive or disable protection.')
                      : c('Ange ditt huvudlösenord för att påbörja aktiveringen av USB-skyddet.', 'Enter your master password to begin setting up USB protection.')}
                  </p>

                  {/* Biometrics removal notice */}
                  {!info?.protected && (
                    <p className="border-l-2 border-[var(--border)] pl-2.5 text-[11px] text-[var(--text-tertiary)]">
                      {c(
                        'Aktiveringen tar bort biometrisk upplåsning. Länkade enheter behöver paras om.',
                        'Enabling this removes biometric unlock. Linked devices need to be paired again.'
                      )}
                    </p>
                  )}

                  {/* Master Password Authorization */}
                  {authFields}

                  {/* Error messages */}
                  {error && <p role="alert" className="break-words text-[11px] text-[var(--destructive)]">{error}</p>}
                  {loadError && <p role="alert" className="break-words text-[11px] text-[var(--destructive)]">{loadError}</p>}

                  {/* Active USB Management — Disable button */}
                  {info?.usb_bound && (
                    <div className="border-t border-[var(--border-subtle)] pt-3 flex items-center justify-between gap-3">
                      <span className="text-[11px] text-[var(--text-tertiary)]">
                        {c('Valvet kan därefter öppnas med endast lösenord.', 'Vault can then be opened with password only.')}
                      </span>
                      <button
                        type="button"
                        className={protectionDanger}
                        disabled={busy || !password}
                        onClick={() => run('unbind')}
                      >
                        {c('Stäng av USB-skydd', 'Disable USB protection')}
                      </button>
                    </div>
                  )}
                </motion.div>
              )}

              {panel === 'usb' && usbStep === 1 && (
                <motion.div
                  key="step-usb"
                  initial={{ opacity: 0, y: 6 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -6 }}
                  transition={{ duration: 0.15 }}
                  className="flex flex-col gap-3"
                >
                  <p className="text-[12px] leading-relaxed text-[var(--text-secondary)]">
                    {c(
                      'Anslut den USB-sticka du vill binda till valvet och välj den i listan. Stickans unika serienummer binds till krypteringen.',
                      'Connect the USB drive you want to bind to the vault and select it below. The drive’s unique serial number is bound to the encryption.'
                    )}
                  </p>

                  {/* USB Picker */}
                  <UsbPicker value={usb} onChange={setUsb} disabled={busy} />

                  <p className="text-[11px] leading-relaxed text-[var(--text-tertiary)]">
                    {c(
                      'När USB-skyddet aktiveras genereras nya återställningsnycklar som du får spara i nästa steg.',
                      'When USB protection is activated, new recovery shares will be generated for you to save in the next step.'
                    )}
                  </p>

                  {/* Error messages */}
                  {error && <p role="alert" className="break-words text-[11px] text-[var(--destructive)]">{error}</p>}
                  {loadError && <p role="alert" className="break-words text-[11px] text-[var(--destructive)]">{loadError}</p>}

                  {/* How it works collapsible */}
                  <details className="text-[11px] text-[var(--text-tertiary)] pt-1 border-t border-[var(--border-subtle)]">
                    <summary className="cursor-pointer hover:text-[var(--text-primary)] transition-colors select-none">
                      {c('Så fungerar skyddet', 'How protection works')}
                    </summary>
                    <p className="mt-1.5 leading-relaxed">
                      {c(
                        'Bindningen använder stickans serienummer och påverkas inte av filnamn eller placering. Serienumret kan förfalskas; skyddet ersätter inte en säker dator.',
                        'Binding uses the drive serial number, regardless of filename or location. Serial numbers can be spoofed; this does not replace a secure computer.'
                      )}
                    </p>
                  </details>
                </motion.div>
              )}

              {panel === 'recovery' && (
                <motion.div
                  key="step-recovery"
                  initial={{ opacity: 0, y: 6 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -6 }}
                  transition={{ duration: 0.15 }}
                  className="flex flex-col gap-3"
                >
                  <p className="text-[12px] leading-relaxed text-[var(--text-secondary)]">
                    {info?.recovery_enabled
                      ? c('Nya nycklar ersätter de gamla för den här valvfilen. Äldre säkerhetskopior behåller sina nycklar.', 'New shares replace the old ones for this vault file. Older backups keep their keys.')
                      : c('Generera tre återställningsnycklar. Två av dem och valvfilen räcker för att återställa åtkomsten om du tappar lösenordet.', 'Generate three recovery shares. Any two and the vault file can restore access if you lose your password.')}
                  </p>

                  {/* Master Password Authorization */}
                  {authFields}

                  {/* Error messages */}
                  {error && <p role="alert" className="break-words text-[11px] text-[var(--destructive)]">{error}</p>}
                  {loadError && <p role="alert" className="break-words text-[11px] text-[var(--destructive)]">{loadError}</p>}

                  {/* Active Recovery Keys Management — Revoke button */}
                  {info?.recovery_enabled && (
                    <div className="border-t border-[var(--border-subtle)] pt-3 flex items-center justify-between gap-3">
                      <span className="text-[11px] text-[var(--text-tertiary)]">
                        {info.usb_bound
                          ? c('Stäng av USB-skyddet innan du återkallar nycklarna.', 'Disable USB protection before revoking the keys.')
                          : c('Återkallade nycklar kan inte återställa den här valvfilen.', 'Revoked keys cannot recover this vault file.')}
                      </span>
                      <button
                        type="button"
                        className={protectionDanger}
                        disabled={busy || !password || info.usb_bound}
                        onClick={() => run('revoke')}
                      >
                        {c('Återkalla nycklar', 'Revoke keys')}
                      </button>
                    </div>
                  )}
                </motion.div>
              )}
            </AnimatePresence>
          </ProtectionPanel>
        </ProtectionDialog>
      )}

      {/* Recovery Kit Wizard Dialog */}
      {kit && (
        <RecoveryKitDialog
          key={kit.verification_hash}
          kit={kit}
          onDone={() => {
            setKit(null);
            if (panel !== 'usb') close();
          }}
          onCancel={cancelKit}
        />
      )}
    </>
  );
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
      className="mt-4 flex flex-col gap-3.5 text-[12px]"
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
        recovering.current = true;
        setBusy(true);
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
      <div className="rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] p-3 text-[12px] flex items-start gap-2.5">
        <Shield size={15} className="mt-0.5 shrink-0 text-[var(--text-secondary)]" />
        <p className="text-[11px] leading-relaxed text-[var(--text-secondary)]">
          {c(
            "Ange två delar från samma recovery-kit. Återställningen byter lösenord, tar bort USB-bindningen och förbrukar detta kit på den uppdaterade filen. Skapa sedan ett nytt kit och bind USB igen.",
            "Enter two shares from the same kit. Recovery changes the password, removes USB binding and consumes this kit for the updated file. Then create a new kit and bind your USB again.",
          )}
        </p>
      </div>

      <div className="flex flex-col gap-2">
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
      </div>

      <div className="flex flex-col gap-2">
        <SecureSecretInput
          value={password}
          onChange={setPassword}
          placeholder={c("Nytt lösenord (minst 12 tecken)", "New password (at least 12 characters)")}
          disabled={busy}
        />
        <SecureSecretInput
          value={confirm}
          onChange={setConfirm}
          placeholder={c("Bekräfta nytt lösenord", "Confirm new password")}
          disabled={busy}
        />
      </div>

      {error && <p role="alert" className="rounded-[3px] border border-[var(--destructive)]/30 bg-[var(--destructive)]/10 p-2.5 text-[11px] text-[var(--destructive)] break-words">{error}</p>}

      <div className="flex flex-col gap-2 pt-1">
        <button
          className={protectionPrimary + ' w-full'}
          disabled={busy || !a.trim() || !b.trim() || a.trim() === b.trim() || !isValidNewMasterPassword(password) || password !== confirm}
          type="submit"
        >
          {busy && <Loader2 size={13} className="animate-spin" />}
          <span>{c("Återställ åtkomst", "Restore access")}</span>
        </button>
        <button className={protectionSecondaryButton + ' w-full'} type="button" disabled={busy} onClick={onBack}>
          {c("Tillbaka", "Back")}
        </button>
      </div>
    </form>
  );
}

