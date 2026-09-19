import React, { useState, useEffect, useRef, useCallback } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, Camera, Image, AlertCircle, RefreshCw, Loader2, CheckCircle2, UploadCloud } from 'lucide-react';
import jsQR from 'jsqr';
import { useTranslation } from '@/contexts/LanguageContext';
import { decodeQrFromImage } from '@/utils/qrDecoder';

export interface QrScannerModalProps {
  isOpen: boolean;
  onClose: () => void;
  onScan: (payload: string) => void;
  title?: string;
  subtitle?: string;
  expectedPrefix?: string;
}

export const QrScannerModal: React.FC<QrScannerModalProps> = ({
  isOpen,
  onClose,
  onScan,
  title,
  subtitle,
  expectedPrefix = 'yntrapair://',
}) => {
  const { t } = useTranslation();
  const [error, setError] = useState<string | null>(null);
  const [cameras, setCameras] = useState<MediaDeviceInfo[]>([]);
  const [selectedCameraId, setSelectedCameraId] = useState<string>('');
  const [isScanning, setIsScanning] = useState<boolean>(true);
  const [isDecodingImage, setIsDecodingImage] = useState<boolean>(false);
  const [isSuccess, setIsSuccess] = useState<boolean>(false);
  const [isDragging, setIsDragging] = useState<boolean>(false);

  const videoRef = useRef<HTMLVideoElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const animationFrameRef = useRef<number | null>(null);
  const hasScannedRef = useRef<boolean>(false);

  // Checks if scanned QR matches expected prefix if configured
  const isValidPayload = useCallback((payload: string): boolean => {
    if (!expectedPrefix) return true;
    return payload.trim().toLowerCase().startsWith(expectedPrefix.toLowerCase());
  }, [expectedPrefix]);

  // Stop camera tracks and clean up
  const stopCamera = useCallback(() => {
    if (animationFrameRef.current) {
      cancelAnimationFrame(animationFrameRef.current);
      animationFrameRef.current = null;
    }
    if (streamRef.current) {
      streamRef.current.getTracks().forEach((track) => track.stop());
      streamRef.current = null;
    }
  }, []);

  // Enumerate available video input devices
  const detectCameras = useCallback(async () => {
    try {
      if (!navigator.mediaDevices?.enumerateDevices) return;
      const devices = await navigator.mediaDevices.enumerateDevices();
      const videoDevices = devices.filter((d) => d.kind === 'videoinput');
      setCameras(videoDevices);
      if (videoDevices.length > 0 && !selectedCameraId) {
        // Prefer back/environment camera on phones
        const backCam = videoDevices.find((d) =>
          d.label.toLowerCase().includes('back') ||
          d.label.toLowerCase().includes('rear') ||
          d.label.toLowerCase().includes('environment')
        );
        setSelectedCameraId(backCam ? backCam.deviceId : videoDevices[0].deviceId);
      }
    } catch {
      // Ignore enumeration failure
    }
  }, [selectedCameraId]);

  // Handle successful scan with feedback
  const handleSuccess = useCallback((payload: string) => {
    if (hasScannedRef.current) return;
    hasScannedRef.current = true;
    setIsSuccess(true);
    stopCamera();

    setTimeout(() => {
      onScan(payload);
    }, 280);
  }, [onScan, stopCamera]);

  // Live video frame scanning loop
  const scanLoop = useCallback(() => {
    if (hasScannedRef.current || !videoRef.current || !canvasRef.current) return;

    const video = videoRef.current;
    const canvas = canvasRef.current;
    const ctx = canvas.getContext('2d', { willReadFrequently: true });

    if (video.readyState === video.HAVE_ENOUGH_DATA && ctx) {
      canvas.width = video.videoWidth;
      canvas.height = video.videoHeight;
      ctx.drawImage(video, 0, 0, canvas.width, canvas.height);

      // 1. Hardware BarcodeDetector if available
      if (typeof window !== 'undefined' && window.BarcodeDetector) {
        try {
          const detector = new window.BarcodeDetector({ formats: ['qr_code'] });
          detector.detect(video).then((barcodes) => {
            if (barcodes && barcodes.length > 0 && !hasScannedRef.current) {
              const rawValue = barcodes[0].rawValue;
              if (rawValue && isValidPayload(rawValue)) {
                handleSuccess(rawValue.trim());
                return;
              }
            }
          }).catch(() => {});
        } catch {
          // Fallback to jsQR
        }
      }

      // 2. Pure JS decode via jsQR
      try {
        const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
        const code = jsQR(imageData.data, imageData.width, imageData.height, {
          inversionAttempts: 'dontInvert',
        });

        if (code && code.data && !hasScannedRef.current) {
          if (isValidPayload(code.data)) {
            handleSuccess(code.data.trim());
            return;
          }
        }
      } catch {
        // Next frame
      }
    }

    if (!hasScannedRef.current) {
      animationFrameRef.current = requestAnimationFrame(scanLoop);
    }
  }, [handleSuccess, isValidPayload]);

  // Start selected camera stream and request permissions
  const startCamera = useCallback(async (deviceId?: string) => {
    stopCamera();
    setError(null);
    hasScannedRef.current = false;
    setIsScanning(true);
    setIsSuccess(false);

    if (!navigator.mediaDevices?.getUserMedia) {
      setError(t('pairing.err_camera_unsupported') || 'Kameran stöds inte i denna miljö eller saknar behörighet. Välj en bild istället.');
      return;
    }

    try {
      const constraints: MediaStreamConstraints = {
        video: deviceId
          ? { deviceId: { exact: deviceId } }
          : {
              facingMode: { ideal: 'environment' },
              width: { ideal: 1280 },
              height: { ideal: 720 },
            },
      };

      const stream = await navigator.mediaDevices.getUserMedia(constraints);
      streamRef.current = stream;

      if (videoRef.current) {
        videoRef.current.srcObject = stream;
        videoRef.current.setAttribute('playsinline', 'true');
        await videoRef.current.play();
        detectCameras();
        animationFrameRef.current = requestAnimationFrame(scanLoop);
      }
    } catch (err: unknown) {
      const errName = (err instanceof Error || (typeof err === 'object' && err !== null && 'name' in err))
        ? (err as { name?: string }).name
        : '';
      const isDenied = errName === 'NotAllowedError' || errName === 'PermissionDeniedError';
      const msg = isDenied
        ? (t('pairing.err_camera_permission') || 'Kamerabehörighet nekades. Tillåt kameraåtkomst i enhetens inställningar eller välj en bild nedan.')
        : (t('pairing.err_camera_init') || 'Kunde inte starta kameran. Kontrollera att ingen annan app använder den.');
      setError(msg);
    }
  }, [detectCameras, scanLoop, stopCamera, t]);

  useEffect(() => {
    if (isOpen) {
      hasScannedRef.current = false;
      setIsSuccess(false);
      setIsDecodingImage(false);
      startCamera(selectedCameraId || undefined);
    } else {
      stopCamera();
    }
    return () => {
      stopCamera();
    };
  }, [isOpen, selectedCameraId, startCamera, stopCamera]);

  // Switch camera between front/back
  const handleSwitchCamera = () => {
    if (cameras.length <= 1) return;
    const currentIndex = cameras.findIndex((c) => c.deviceId === selectedCameraId);
    const nextIndex = (currentIndex + 1) % cameras.length;
    const nextId = cameras[nextIndex].deviceId;
    setSelectedCameraId(nextId);
    startCamera(nextId);
  };

  // Robust decoding of image files (photos, gallery, screenshots)
  const handleProcessFile = useCallback(async (file: File | Blob) => {
    setError(null);
    setIsDecodingImage(true);

    try {
      const result = await decodeQrFromImage(file);

      if (result.success && result.text) {
        const code = result.text.trim();
        if (isValidPayload(code)) {
          handleSuccess(code);
        } else {
          setError(
            t('pairing.err_wrong_qr_type') ||
            'Hittade en QR-kod, men den är inte en giltig Yntra Vault-parkopplingskod.'
          );
        }
      } else {
        setError(
          t('pairing.err_no_qr_found') ||
          'Ingen giltig QR-kod kunde upptäckas i den valda bilden. Se till att QR-koden är skarp och väl synlig.'
        );
      }
    } catch {
      setError(
        t('pairing.err_decode_failed') ||
        'Kunde inte läsa bilden. Försök med en annan bild eller rikta kameran direkt mot QR-koden.'
      );
    } finally {
      setIsDecodingImage(false);
    }
  }, [handleSuccess, isValidPayload, t]);

  // Handle file input change
  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    handleProcessFile(file);
    e.target.value = '';
  };

  // Drag and drop image files onto the viewfinder
  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(true);
  };

  const handleDragLeave = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);
    const file = e.dataTransfer.files?.[0];
    if (file) {
      handleProcessFile(file);
    }
  };

  // Clipboard paste support (e.g. pasted screenshot)
  useEffect(() => {
    if (!isOpen) return;

    const handlePaste = (e: ClipboardEvent) => {
      const items = e.clipboardData?.items;
      if (!items) return;
      for (let i = 0; i < items.length; i++) {
        if (items[i].type.startsWith('image/')) {
          const file = items[i].getAsFile();
          if (file) {
            handleProcessFile(file);
            break;
          }
        }
      }
    };

    window.addEventListener('paste', handlePaste);
    return () => window.removeEventListener('paste', handlePaste);
  }, [isOpen, handleProcessFile]);

  return (
    <AnimatePresence>
      {isOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 bg-black/75 backdrop-blur-sm"
            onClick={onClose}
          />

          <motion.div
            initial={{ opacity: 0, scale: 0.96 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.96 }}
            className="relative w-full max-w-[420px] rounded-[4px] border border-[var(--border)] bg-[var(--bg-elevated)] shadow-2xl overflow-hidden z-10"
            onDragOver={handleDragOver}
            onDragLeave={handleDragLeave}
            onDrop={handleDrop}
          >
            {/* Header */}
            <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border-subtle)] bg-[var(--bg-surface)]">
              <div className="flex items-center gap-2">
                <div className="h-7 w-7 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] flex items-center justify-center text-[var(--text-secondary)]">
                  <Camera className="w-3.5 h-3.5" />
                </div>
                <div>
                  <h3 className="text-sm font-semibold text-[var(--text-primary)]">
                    {title || t('pairing.scan_qr_title') || 'Skanna QR-kod'}
                  </h3>
                  <p className="text-[11px] text-[var(--text-muted)]">
                    {subtitle || t('pairing.scan_qr_subtitle') || 'Rikta kameran mot QR-koden eller välj en bild'}
                  </p>
                </div>
              </div>
              <button
                type="button"
                onClick={onClose}
                className="h-7 w-7 rounded-[3px] border border-transparent hover:border-[var(--border)] flex items-center justify-center text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            {/* Viewfinder Body */}
            <div className="relative w-full aspect-square bg-black overflow-hidden flex items-center justify-center">
              <video
                ref={videoRef}
                className="absolute inset-0 w-full h-full object-cover"
                playsInline
                muted
              />
              <canvas ref={canvasRef} className="hidden" />

              {/* Viewfinder Target Reticle Frame */}
              <div className="relative z-10 w-[210px] h-[210px] pointer-events-none">
                {/* Corner Accents */}
                <div className="absolute top-0 left-0 w-7 h-7 border-t-2 border-l-2 border-white/90 rounded-tl-[3px]" />
                <div className="absolute top-0 right-0 w-7 h-7 border-t-2 border-r-2 border-white/90 rounded-tr-[3px]" />
                <div className="absolute bottom-0 left-0 w-7 h-7 border-b-2 border-l-2 border-white/90 rounded-bl-[3px]" />
                <div className="absolute bottom-0 right-0 w-7 h-7 border-b-2 border-r-2 border-white/90 rounded-br-[3px]" />

                {/* Animated Laser Scan Line */}
                {isScanning && !error && !isDecodingImage && !isSuccess && (
                  <motion.div
                    animate={{ y: [0, 204, 0] }}
                    transition={{ repeat: Infinity, duration: 2.2, ease: 'easeInOut' }}
                    className="w-full h-[1.5px] bg-emerald-400 shadow-[0_0_8px_rgba(52,211,153,0.8)]"
                  />
                )}
              </div>

              {/* Dragging Overlay */}
              {isDragging && (
                <div className="absolute inset-0 z-30 flex flex-col items-center justify-center bg-emerald-950/80 backdrop-blur-xs border-2 border-dashed border-emerald-400 text-center p-4 pointer-events-none">
                  <UploadCloud className="w-10 h-10 text-emerald-400 mb-2 animate-bounce" />
                  <p className="text-xs font-semibold text-white">
                    {t('pairing.drop_qr_image') || 'Släpp bilden här för att skanna'}
                  </p>
                </div>
              )}

              {/* Decoding Image Loading Overlay */}
              {isDecodingImage && (
                <div className="absolute inset-0 z-30 flex flex-col items-center justify-center bg-black/85 backdrop-blur-xs text-center p-6">
                  <Loader2 className="w-9 h-9 text-emerald-400 animate-spin mb-3" />
                  <p className="text-xs font-medium text-white mb-1">
                    {t('pairing.decoding_image') || 'Analyserar och läser QR-kod...'}
                  </p>
                  <p className="text-[11px] text-zinc-400">
                    {t('pairing.decoding_subtext') || 'Optimerar bild för maximal avkodning'}
                  </p>
                </div>
              )}

              {/* Success Flash Overlay */}
              {isSuccess && (
                <motion.div
                  initial={{ opacity: 0, scale: 0.9 }}
                  animate={{ opacity: 1, scale: 1 }}
                  className="absolute inset-0 z-30 flex flex-col items-center justify-center bg-emerald-950/90 text-center p-6"
                >
                  <div className="w-12 h-12 rounded-full bg-emerald-500/20 border border-emerald-500/40 flex items-center justify-center text-emerald-400 mb-2">
                    <CheckCircle2 className="w-7 h-7" />
                  </div>
                  <p className="text-xs font-semibold text-white">
                    {t('pairing.qr_scanned_verified') || 'QR-kod identifierad!'}
                  </p>
                </motion.div>
              )}

              {/* Error Overlay with Clear Action Prompts */}
              {error && !isDecodingImage && !isSuccess && (
                <div className="absolute inset-0 z-20 flex flex-col items-center justify-center p-6 bg-black/90 text-center">
                  <AlertCircle className="w-8 h-8 text-red-400 mb-2 shrink-0" />
                  <p className="text-xs text-zinc-200 font-medium max-w-[280px] mb-4 leading-relaxed">
                    {error}
                  </p>
                  <div className="flex flex-wrap items-center justify-center gap-2">
                    <button
                      type="button"
                      onClick={() => startCamera(selectedCameraId || undefined)}
                      className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-white bg-zinc-800 hover:bg-zinc-700 rounded-[3px] border border-zinc-700 transition-colors cursor-pointer"
                    >
                      <RefreshCw className="w-3.5 h-3.5" />
                      {t('common.retry') || 'Begär behörighet / Försök igen'}
                    </button>
                    <button
                      type="button"
                      onClick={() => fileInputRef.current?.click()}
                      className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-zinc-900 bg-emerald-400 hover:bg-emerald-300 rounded-[3px] transition-colors cursor-pointer shadow-xs"
                    >
                      <Image className="w-3.5 h-3.5" />
                      {t('pairing.pick_qr_image') || 'Välj bild eller ta foto'}
                    </button>
                  </div>
                </div>
              )}
            </div>

            {/* Footer Actions */}
            <div className="flex items-center justify-between px-4 py-3 border-t border-[var(--border-subtle)] bg-[var(--bg-surface)]">
              {cameras.length > 1 ? (
                <button
                  type="button"
                  onClick={handleSwitchCamera}
                  className="flex items-center gap-1.5 px-2.5 py-1.5 text-xs font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] rounded-[3px] border border-[var(--border)] hover:bg-[var(--bg-elevated)] transition-colors cursor-pointer"
                >
                  <RefreshCw className="w-3.5 h-3.5" />
                  {t('pairing.switch_camera') || 'Växla kamera'}
                </button>
              ) : (
                <div />
              )}

              {/* Select image / Take photo button */}
              <div>
                <input
                  ref={fileInputRef}
                  type="file"
                  accept="image/*"
                  onChange={handleFileChange}
                  className="hidden"
                />
                <button
                  type="button"
                  onClick={() => fileInputRef.current?.click()}
                  className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-[var(--text-primary)] hover:text-[var(--text-primary)] rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] hover:bg-[var(--bg-elevated)] transition-colors cursor-pointer shadow-2xs"
                >
                  <Image className="w-3.5 h-3.5 text-emerald-400" />
                  <span>{t('pairing.pick_qr_image') || 'Välj bild eller ta foto'}</span>
                </button>
              </div>
            </div>
          </motion.div>
        </div>
      )}
    </AnimatePresence>
  );
};
