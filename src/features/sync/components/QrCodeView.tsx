import React, { useState, useEffect } from 'react';
import { QRCodeSVG } from 'qrcode.react';
import { RotateCw, ShieldCheck, Clock } from 'lucide-react';
import { useTranslation } from '@/contexts/LanguageContext';

export interface QrCodeViewProps {
  payload: string;
  sasCode: string;
  expiresAt: number; // Unix timestamp in seconds
  onRefresh?: () => void;
  className?: string;
}

export const QrCodeView: React.FC<QrCodeViewProps> = ({
  payload,
  sasCode,
  expiresAt,
  onRefresh,
  className = '',
}) => {
  const { t } = useTranslation();
  const [timeLeft, setTimeLeft] = useState<number>(() => {
    const remaining = Math.max(0, expiresAt - Math.floor(Date.now() / 1000));
    return remaining;
  });

  useEffect(() => {
    const interval = setInterval(() => {
      const remaining = Math.max(0, expiresAt - Math.floor(Date.now() / 1000));
      setTimeLeft(remaining);
    }, 1000);
    return () => clearInterval(interval);
  }, [expiresAt]);

  const isExpired = timeLeft <= 0;
  const progressPercent = Math.min(100, Math.max(0, (timeLeft / 90) * 100));

  return (
    <div className={`flex flex-col items-center justify-center ${className}`}>
      {/* QR Container */}
      <div className="relative p-3.5 rounded-[4px] border border-[var(--border)] bg-white shadow-sm transition-all">
        {isExpired ? (
          <div className="w-[190px] h-[190px] flex flex-col items-center justify-center text-center p-4 bg-zinc-50 rounded-[3px]">
            <Clock className="w-8 h-8 text-zinc-400 mb-2" />
            <p className="text-xs text-zinc-700 font-medium mb-3">
              {t('pairing.qr_expired') || 'QR-koden har löpt ut'}
            </p>
            {onRefresh && (
              <button
                type="button"
                onClick={onRefresh}
                className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-white bg-zinc-900 hover:bg-zinc-800 rounded-[3px] transition-colors"
              >
                <RotateCw className="w-3.5 h-3.5" />
                {t('pairing.qr_refresh') || 'Generera ny'}
              </button>
            )}
          </div>
        ) : (
          <QRCodeSVG
            value={payload}
            size={190}
            level="M"
            includeMargin={false}
            fgColor="#09090b"
            bgColor="#ffffff"
          />
        )}
      </div>

      {/* SAS Confirmation Badge */}
      <div className="mt-3.5 flex items-center justify-center gap-2">
        <div className="flex items-center gap-1.5 px-3 py-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-xs">
          <ShieldCheck className="w-3.5 h-3.5 text-[var(--text-secondary)]" />
          <span className="text-[var(--text-muted)] font-mono">
            {t('pairing.sas_label') || 'Bekräftelsekod:'}
          </span>
          <span className="font-mono font-bold tracking-widest text-[var(--text-primary)]">
            {sasCode}
          </span>
        </div>
      </div>

      {/* Countdown Timer */}
      <div className="mt-2.5 w-full max-w-[218px]">
        <div className="flex items-center justify-between text-[11px] text-[var(--text-muted)] mb-1 font-mono">
          <span>{t('pairing.qr_valid_for') || 'Giltig i:'}</span>
          <span className={timeLeft < 15 ? 'text-red-500 font-bold' : ''}>
            {timeLeft}s
          </span>
        </div>
        <div className="w-full h-1 bg-[var(--border)] rounded-full overflow-hidden">
          <div
            className={`h-full transition-all duration-1000 ${
              timeLeft < 15 ? 'bg-red-500' : 'bg-[var(--text-primary)]'
            }`}
            style={{ width: `${progressPercent}%` }}
          />
        </div>
      </div>
    </div>
  );
};
