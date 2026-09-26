import { useCapabilities } from '@/lib/platform';
import { RecoveryKitDialog } from './RecoveryKitWizard';
export { RecoveryKitCards } from './RecoveryKitWizard';
import { useEffect, useState } from "react";
import {
  getBackend,
  openFileDialog,
  type EmergencyKit,
  type VaultInfo,
} from "@/lib/backend";
import { useTranslation } from "@/contexts/LanguageContext";
import { SecureSecretInput } from "@/components/ui";

export interface UsbStorageDevice {
  id: string;
  name: string;
}
export interface LocalProtectionInfo {
  protected: boolean;
  usb_bound: boolean;
  recovery_enabled: boolean;
}
const field =
  "w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] p-2 text-[12px] text-[var(--text-primary)]";
const button =
  "rounded-[3px] border border-[var(--border)] px-3 py-2 text-[12px] hover:bg-[var(--bg-hover)] disabled:opacity-50";
function useCopy() {
  const { language } = useTranslation();
  return (sv: string, en: string) => (language === "sv" ? sv : en);
}

export function UsbPicker({
  value,
  onChange,
  disabled = false,
}: {
  value: string;
  onChange: (id: string) => void;
  disabled?: boolean;
}) {
  const [devices, setDevices] = useState<UsbStorageDevice[]>([]);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const c = useCopy();
  const refresh = async () => {
    setBusy(true);
    setError("");
    try {
      setDevices(await (await getBackend()).listUsbStorageDevices());
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  useEffect(() => {
    void refresh();
  }, []);
  return (
    <div className="flex flex-col gap-2">
      <label>
        {c("USB-sticka", "USB drive")}
        <select
          aria-label={c("USB-sticka", "USB drive")}
          className={field}
          value={value}
          disabled={disabled || busy}
          onChange={(e) => onChange(e.target.value)}
        >
          <option value="">
            {c("Välj en ansluten sticka", "Select a connected drive")}
          </option>
          {devices.map((d) => (
            <option key={d.id} value={d.id}>
              {d.name} · {d.id.slice(0, 8)}
            </option>
          ))}
        </select>
      </label>
      <button
        type="button"
        className={button}
        disabled={busy || disabled}
        onClick={refresh}
      >
        {c("Sök igen", "Refresh")}
      </button>
      {!busy && !devices.length && (
        <p>
          {c(
            "Ingen kompatibel USB-sticka hittades. USB-bindning stöds på Windows.",
            "No compatible USB drive found. USB binding is supported on Windows.",
          )}
        </p>
      )}
      {error && <p role="alert">{error}</p>}
    </div>
  );
}

export function LocalProtectionSettings({
  onProtectionChanged,
}: { onProtectionChanged?: (info: LocalProtectionInfo) => void } = {}) {
  const c = useCopy();
  const [info, setInfo] = useState<LocalProtectionInfo | null>(null);
  const [password, setPassword] = useState("");
  const [keyfile, setKeyfile] = useState("");
  const capabilities = useCapabilities();
  const [usb, setUsb] = useState("");
  const [kit, setKit] = useState<EmergencyKit | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const refresh = async () => {
    const current = await (await getBackend()).getLocalProtection();
    setInfo(current);
    onProtectionChanged?.(current);
  };
  useEffect(() => {
    refresh().catch((e) => setError(String(e)));
  }, []);
  const run = async (action: "generate" | "bind" | "unbind" | "revoke") => {
    setError("");
    setBusy(true);
    try {
      const b = await getBackend();
      if (action === "generate")
        setKit(await b.generateEmergencyKit(password, keyfile || undefined));
      else if (action === "revoke")
        await b.revokeRecovery(password, keyfile || undefined);
      else
        await b.setUsbBinding(
          password,
          keyfile || undefined,
          action === "bind" ? usb : undefined,
        );
      setPassword("");
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <section className="flex flex-col gap-3 text-[12px] rounded-[3px] border border-[var(--border)] p-4">
      <h3 className="font-semibold">
        {capabilities.usbBinding ? c("USB-bindning och återställning", "USB binding and recovery") : c("Återställning", "Recovery")}
      </h3>
      {capabilities.usbBinding && <>
      <p>
        {info?.usb_bound
          ? c(
              "USB-bindning är aktiv på den här valvkopian.",
              "USB binding is active on this vault copy.",
            )
          : c("USB-bindning är avstängd.", "USB binding is off.")}
      </p>
      <p>
        {c(
          "Filer kan flyttas och byta namn. Bindningen använder hårdvarans serienummer, inte diskens formatering. Identifieraren kan förfalskas; detta är inget fullständigt skydd mot skadlig kod.",
          "Files may be moved and renamed. Binding uses the hardware serial, not filesystem formatting. Identifiers can be spoofed; this does not fully protect against malware.",
        )}
      </p>
      </>}
      <p>
        {c(
          "Recovery v2 ersätter lösenordsdelarna med en separat återställningshemlighet. Första aktiveringen kräver att länkade enheter paras om och tar bort biometrisk upplåsning. Biometri och äldre säkerhetsnycklar kan ännu inte kombineras med detta format. Stäng av äldre hårdvaru-2FA före aktivering.",
          "Recovery v2 replaces password shares with a separate recovery secret. First activation requires re-pairing linked devices and removes biometric unlock. Biometrics and legacy security-key enrollment cannot yet be combined with this format. Disable legacy hardware 2FA before activation.",
        )}
      </p>
      <SecureSecretInput
        value={password}
        onChange={setPassword}
        placeholder={c("Nuvarande huvudlösenord", "Current master password")}
        disabled={busy}
      />
      <button
        type="button"
        className={button}
        disabled={busy}
        onClick={async () => {
          const p = await openFileDialog({ multiple: false });
          if (typeof p === "string") setKeyfile(p);
        }}
      >
        {keyfile
          ? c("Nyckelfil vald", "Key file selected")
          : c(
              "Välj nyckelfil om valvet kräver en",
              "Select key file if required",
            )}
      </button>
      {error && (
        <p role="alert" className="text-[var(--destructive)]">
          {error}
        </p>
      )}
      <button
        type="button"
        className={button}
        disabled={busy || !password || !!kit}
        onClick={() => run("generate")}
      >
        {info?.recovery_enabled
          ? c(
              "Ersätt recovery-kit — gamla delar återkallas",
              "Replace recovery kit — revoke old shares",
            )
          : c("Skapa recovery-kit", "Create recovery kit")}
      </button>
      {kit ? (
        <RecoveryKitDialog
          key={kit.verification_hash}
          kit={kit}
          onDone={() => setKit(null)}
        />
      ) : (
        <>
          {capabilities.usbBinding && <>
          <UsbPicker value={usb} onChange={setUsb} disabled={busy} />
          <button
            type="button"
            className={button}
            disabled={busy || !password || !usb || !info?.recovery_enabled}
            onClick={() => run("bind")}
          >
            {c("Bind till vald USB-sticka", "Bind to selected USB drive")}
          </button>
          {info?.usb_bound && (
            <button
              type="button"
              className={button}
              disabled={busy || !password}
              onClick={() => run("unbind")}
            >
              {c("Ta bort USB-bindning", "Remove USB binding")}
            </button>
          )}
          </>}
          {info?.recovery_enabled && !info.usb_bound && (
            <button
              type="button"
              className={button}
              disabled={busy || !password}
              onClick={() => run("revoke")}
            >
              {c("Återkalla recovery-kit", "Revoke recovery kit")}
            </button>
          )}
        </>
      )}
    </section>
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
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  return (
    <form
      className="mt-4 flex flex-col gap-3 text-[12px]"
      onSubmit={async (e) => {
        e.preventDefault();
        setError("");
        if (password.length < 12 || password !== confirm) {
          setError(
            c(
              "Ange samma nya lösenord två gånger, minst 12 tecken.",
              "Enter the same new password twice, at least 12 characters.",
            ),
          );
          return;
        }
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
        disabled={busy || !a || !b || !password || password !== confirm}
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
