import { useRef, useState } from 'react';
import { ArrowLeft, ArrowRight, Check, Copy, Download, Eye, EyeOff, KeyRound, Loader2 } from 'lucide-react';
import { getBackend, type EmergencyKit } from '@/lib/backend';
import { useTranslation } from '@/contexts/LanguageContext';
import { ActionTooltip } from '@/components/ui/tooltip';
import {
  ProtectionDialog,
  ProtectionPanel,
  protectionPrimary,
  protectionSecondaryButton,
} from './ProtectionDialog';

export function RecoveryKitCards({
  kit,
  onDone,
  onCancel,
}: {
  kit: EmergencyKit;
  onDone?: () => void;
  onCancel?: () => void;
}) {
  const { language } = useTranslation();
  const c = (sv: string, en: string) => (language === 'sv' ? sv : en);
  const [step, setStep] = useState(0);
  const [shareIndex, setShareIndex] = useState(0);
  const [saved, setSaved] = useState<number[]>([]);
  const [visible, setVisible] = useState(false);
  const [busy, setBusy] = useState(false);
  const inFlight = useRef(false);
  const [error, setError] = useState('');
  const [exported, setExported] = useState<number[]>([]);
  const [copied, setCopied] = useState(false);
  const copyTimeout = useRef<number | null>(null);

  const share = kit.shares[shareIndex];
  const savedCount = new Set(saved).size;
  const titles = [c('Förbered', 'Prepare'), c('Spara', 'Save'), c('Klart', 'Review')];

  const chooseShare = (index: number) => {
    setShareIndex(index);
    setVisible(false);
    setError('');
  };

  const exportShare = async () => {
    if (!share || inFlight.current) return;
    inFlight.current = true;
    setError('');
    setBusy(true);
    try {
      const backend = await getBackend();
      const path = await backend.saveFileDialog({
        defaultPath: 'yntra-recovery-' + kit.verification_hash + '-share-' + share.share_index + '.txt',
        filters: [{ name: 'Recovery share', extensions: ['txt'] }],
      });
      if (path) {
        await backend.exportRecoveryShare(path, share.share_data);
        setExported((values) => [...new Set([...values, share.share_index])]);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      inFlight.current = false;
      setBusy(false);
    }
  };

  const copyShareText = async () => {
    if (!share) return;
    try {
      await navigator.clipboard.writeText(share.share_data);
      setCopied(true);
      if (copyTimeout.current) window.clearTimeout(copyTimeout.current);
      copyTimeout.current = window.setTimeout(() => setCopied(false), 2000);
    } catch {
      // Fallback
    }
  };

  const stepper = (
    <div className="flex items-center justify-between gap-2">
      {titles.map((title, idx) => (
        <div key={title} className="flex flex-1 flex-col items-center gap-1.5 min-w-0">
          <div
            className={`h-1 w-full rounded-full transition-colors ${
              idx <= step ? 'bg-[var(--text-primary)]' : 'bg-[var(--border-subtle)]'
            }`}
          />
          <span
            className={`text-[10px] font-medium transition-colors whitespace-nowrap truncate ${
              idx === step ? 'text-[var(--text-primary)]' : 'text-[var(--text-tertiary)]'
            }`}
          >
            {title}
          </span>
        </div>
      ))}
    </div>
  );

  const footer = (
    <>
      <div className="flex items-center gap-2">
        {step > 0 ? (
          <button
            type="button"
            className={protectionSecondaryButton}
            disabled={busy}
            onClick={() => {
              setVisible(false);
              setStep(step - 1);
            }}
          >
            <ArrowLeft size={13} />
            <span>{c('Tillbaka', 'Back')}</span>
          </button>
        ) : (
          <span className="text-[11px] text-[var(--text-tertiary)]">
            {c('2 av 3 nycklar behövs', '2 of 3 shares required')}
          </span>
        )}
        {onCancel && (
          <button
            type="button"
            className={protectionSecondaryButton}
            disabled={busy}
            onClick={onCancel}
          >
            {c('Avbryt', 'Cancel')}
          </button>
        )}
      </div>
      {step === 0 ? (
        <button
          type="button"
          className={protectionPrimary}
          onClick={() => setStep(1)}
        >
          <span>{c('Börja spara', 'Start saving')}</span>
          <ArrowRight size={13} />
        </button>
      ) : step === 1 ? (
        <button
          type="button"
          className={protectionPrimary}
          disabled={busy || savedCount < 2}
          onClick={() => {
            setVisible(false);
            setStep(2);
          }}
        >
          <span>{c('Fortsätt', 'Continue')}</span>
          <ArrowRight size={13} />
        </button>
      ) : (
        <button
          type="button"
          className={protectionPrimary}
          disabled={savedCount < 2 || busy}
          onClick={onDone}
        >
          <Check size={13} />
          <span>{c('Klart', 'Done')}</span>
        </button>
      )}
    </>
  );

  return (
    <ProtectionPanel
      title={c('Återställningsnycklar', 'Recovery keys')}
      subtitle={kit.vault_name}
      icon={<KeyRound size={14} />}
      stepper={stepper}
      onClose={onCancel}
      footer={footer}
    >
      <div className="flex flex-col gap-4">
        {step === 0 && (
          <div className="flex flex-col gap-3">
            <p className="text-[12px] leading-relaxed text-[var(--text-secondary)]">
              {c(
                'Två av de tre nycklarna och din valvfil återställer åtkomsten om du förlorar lösenordet eller USB-stickan.',
                'Two different shares and your vault file restore access if you lose your password or USB drive.'
              )}
            </p>

            <div className="flex flex-col gap-2 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] p-3 text-[11px] text-[var(--text-secondary)]">
              <div className="flex items-center gap-2">
                <Check size={13} className="text-[var(--text-primary)] shrink-0" />
                <span>{c('Spara nycklarna på separata, säkra platser.', 'Store the shares in separate, safe places.')}</span>
              </div>
              <div className="flex items-center gap-2 border-t border-[var(--border-subtle)] pt-2">
                <Check size={13} className="text-[var(--text-primary)] shrink-0" />
                <span>{c('Behåll också en säkerhetskopia av valvfilen.', 'Also keep a backup of your vault file.')}</span>
              </div>
            </div>

            <p className="text-[11px] leading-relaxed text-[var(--text-tertiary)]">
              {c(
                'Nycklarna visas bara nu. Äldre nycklar kan fortfarande öppna äldre säkerhetskopior.',
                'These shares are shown only now. Older shares may still open older backups.'
              )}
            </p>
          </div>
        )}

        {step === 1 && share && (
          <div className="flex flex-col gap-3.5">
            {/* Share Switcher Tabs */}
            <div
              role="group"
              aria-label={c('Välj nyckel', 'Choose recovery share')}
              className="flex gap-1.5 p-0.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)]"
            >
              {kit.shares.map((item, index) => {
                const isSelected = index === shareIndex;
                const isSaved = saved.includes(item.share_index);
                return (
                  <button
                    type="button"
                    key={item.share_index}
                    disabled={busy}
                    aria-pressed={isSelected}
                    onClick={() => chooseShare(index)}
                    className={`flex-1 flex items-center justify-center gap-1.5 rounded-[2px] py-1.5 text-[11px] font-medium transition-colors cursor-pointer select-none ${
                      isSelected
                        ? 'bg-[var(--bg-elevated)] text-[var(--text-primary)] shadow-sm'
                        : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)]'
                    }`}
                  >
                    {isSaved && <Check size={12} className="text-[var(--text-primary)]" />}
                    <span>{c('Nyckel', 'Share')} {item.share_index}</span>
                  </button>
                );
              })}
            </div>

            {/* Share Detail Card */}
            <div className="rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] p-3.5 flex flex-col gap-3">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2 text-[var(--text-primary)] font-medium text-[12px]">
                  <KeyRound size={14} className="text-[var(--text-secondary)]" />
                  <span>{c('Nyckel', 'Share')} {share.share_index}</span>
                </div>
                {exported.includes(share.share_index) ? (
                  <span className="inline-flex items-center gap-1 text-[10px] font-medium text-[var(--text-primary)] bg-[var(--bg-elevated)] px-2 py-0.5 rounded-[3px] border border-[var(--border)]">
                    <Check size={11} /> {c('Fil sparad', 'File saved')}
                  </span>
                ) : (
                  <span className="text-[10px] text-[var(--text-tertiary)]">
                    {c('Inte nedladdad än', 'Not downloaded yet')}
                  </span>
                )}
              </div>

              <button
                type="button"
                className="h-8 w-full inline-flex items-center justify-center gap-2 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer disabled:opacity-50"
                disabled={busy}
                onClick={exportShare}
              >
                {busy ? <Loader2 size={13} className="animate-spin" /> : <Download size={13} />}
                <span>{c('Spara nyckeln', 'Save this share')}</span>
              </button>

              {exported.includes(share.share_index) && (
                <p role="status" className="text-[11px] text-[var(--text-secondary)]">
                  {c('Sparad. Förvara den åtskild från övriga nycklar.', 'File saved. Keep it separate from the other shares.')}
                </p>
              )}

              <div className="border-t border-[var(--border-subtle)] pt-2.5">
                <div className="flex items-center justify-between">
                  <button
                    type="button"
                    className="inline-flex items-center gap-1.5 text-[11px] font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors cursor-pointer"
                    aria-expanded={visible}
                    onClick={() => setVisible((v) => !v)}
                  >
                    {visible ? <EyeOff size={13} /> : <Eye size={13} />}
                    <span>{visible ? c('Dölj nyckeln', 'Hide share') : c('Visa för avskrift', 'Show for transcription')}</span>
                  </button>
                  {visible && (
                    <ActionTooltip content={copied ? c('Kopierad!', 'Copied!') : c('Kopiera nyckel', 'Copy share')}>
                      <button
                        type="button"
                        onClick={copyShareText}
                        className="inline-flex items-center gap-1 rounded-[3px] px-1.5 py-0.5 text-[11px] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                      >
                        {copied ? <Check size={12} className="text-[var(--text-primary)]" /> : <Copy size={12} />}
                        <span>{copied ? c('Kopierad', 'Copied') : c('Kopiera', 'Copy')}</span>
                      </button>
                    </ActionTooltip>
                  )}
                </div>
                {visible && (
                  <div className="mt-2">
                    <code className="block max-h-24 overflow-y-auto whitespace-pre-wrap break-all rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-2.5 font-mono text-[10px] text-[var(--text-primary)] select-all leading-normal">
                      {share.share_data}
                    </code>
                  </div>
                )}
              </div>
            </div>

            {/* Confirmation Checkbox */}
            <label className="flex cursor-pointer items-start gap-2.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] p-3 text-[12px] text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors">
              <input
                className="mt-0.5 accent-[var(--text-primary)]"
                type="checkbox"
                disabled={busy}
                checked={saved.includes(share.share_index)}
                onChange={(event) =>
                  setSaved((values) =>
                    event.target.checked
                      ? [...new Set([...values, share.share_index])]
                      : values.filter((index) => index !== share.share_index)
                  )
                }
              />
              <span className="text-[11px] leading-relaxed">
                {c('Jag har sparat den här nyckeln på en säker plats.', 'I have stored this share in a safe place.')}
              </span>
            </label>

            {/* Progress summary & Next */}
            <div className="flex items-center justify-between text-[11px] text-[var(--text-secondary)] px-0.5">
              <span aria-live="polite">
                {savedCount} / 3 {c('bekräftade (minst 2 krävs)', 'confirmed (at least 2 required)')}
              </span>
              {shareIndex < kit.shares.length - 1 && (
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => chooseShare(shareIndex + 1)}
                  className="inline-flex items-center gap-1 font-medium text-[var(--text-primary)] hover:underline cursor-pointer"
                >
                  <span>{c('Nästa nyckel', 'Next share')}</span>
                  <ArrowRight size={12} />
                </button>
              )}
            </div>
          </div>
        )}

        {step === 2 && (
          <div className="flex flex-col gap-3">
            <p className="text-[12px] leading-relaxed text-[var(--text-secondary)]">
              {c(
                'Dina återställningsnycklar är sparade. Förvara dem separat från varandra och valvet.',
                'Your recovery shares are saved. Keep them separate from each other and your vault.'
              )}
            </p>

            <div className="divide-y divide-[var(--border-subtle)] rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 text-[12px]">
              {kit.shares.map((item) => (
                <div key={item.share_index} className="flex items-center justify-between py-2.5">
                  <span className="font-medium text-[var(--text-primary)]">
                    {c('Nyckel', 'Share')} {item.share_index}
                  </span>
                  <span className="flex items-center gap-1.5 text-[11px] text-[var(--text-secondary)]">
                    {saved.includes(item.share_index) ? (
                      <>
                        <Check size={13} className="text-[var(--text-primary)]" />
                        <span>{c('Bekräftad', 'Confirmed')}</span>
                      </>
                    ) : (
                      <span className="text-[var(--text-tertiary)]">{c('Inte bekräftad', 'Not confirmed')}</span>
                    )}
                  </span>
                </div>
              ))}
            </div>

            <p className="text-[11px] leading-relaxed text-[var(--text-tertiary)]">
              {c('Efter att du stänger går det inte att visa nycklarna igen.', 'These shares cannot be shown again after closing.')}
            </p>
          </div>
        )}

        {error && <p role="alert" className="break-words text-[11px] text-[var(--destructive)]">{error}</p>}
      </div>
    </ProtectionPanel>
  );
}

export function RecoveryKitDialog({ kit, onDone, onCancel }: { kit: EmergencyKit; onDone: () => void; onCancel?: () => void }) {
  const { language } = useTranslation();
  return (
    <ProtectionDialog label={language === 'sv' ? 'Spara återställningsnycklar' : 'Save recovery shares'} onClose={onCancel}>
      <RecoveryKitCards key={kit.verification_hash} kit={kit} onDone={onDone} onCancel={onCancel} />
    </ProtectionDialog>
  );
}

