import React, { useState, useEffect, useRef, useCallback } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, Camera, Image, AlertCircle, RefreshCw } from 'lucide-react';
import jsQR from 'jsqr';
import { useTranslation } from '@/contexts/LanguageContext';

export interface QrScannerModalProps {
  isOpen: boolean;
  onClose: () => void;
  onScan: (payload: string) => void;
}

export const QrScannerModal: React.FC<QrScannerModalProps> = ({
  isOpen,
  onClose,
  onScan,
}) => {
  const { t } = useTranslation();
  const [error, setError] = useState<string | null>(null);
  const [cameras, setCameras] = useState<MediaDeviceInfo[]>([]);
  const [selectedCameraId, setSelectedCameraId] = useState<string>('');
  const [isScanning, setIsScanning] = useState<boolean>(true);

  const videoRef = useRef<HTMLVideoElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const animationFrameRef = useRef<number | null>(null);
  const hasScannedRef = useRef<boolean>(false);

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
        // Prefer back/environment camera if available
        const backCam = videoDevices.find((d) =>
          d.label.toLowerCase().includes('back') || d.label.toLowerCase().includes('rear') || d.label.toLowerCase().includes('environment')
        );
        setSelectedCameraId(backCam ? backCam.deviceId : videoDevices[0].deviceId);
      }
    } catch {
      // Ignore enumeration failure
    }
  }, [selectedCameraId]);

  // Scan frame loop
  const scanLoop = useCallback(() => {
    if (hasScannedRef.current || !videoRef.current || !canvasRef.current) return;

    const video = videoRef.current;
    const canvas = canvasRef.current;
    const ctx = canvas.getContext('2d', { willReadFrequently: true });

    if (video.readyState === video.HAVE_ENOUGH_DATA && ctx) {
      canvas.width = video.videoWidth;
      canvas.height = video.videoHeight;
      ctx.drawImage(video, 0, 0, canvas.width, canvas.height);

      // 1. Try Hardware-accelerated BarcodeDetector if available
      if ('BarcodeDetector' in window) {
        try {
          const detector = new (window as any).BarcodeDetector({ formats: ['qr_code'] });
          detector.detect(video).then((barcodes: any[]) => {
            if (barcodes && barcodes.length > 0 && !hasScannedRef.current) {
              const rawValue = barcodes[0].rawValue;
              if (rawValue && rawValue.startsWith('yntrapair://')) {
                hasScannedRef.current = true;
                stopCamera();
                onScan(rawValue);
                return;
              }
            }
          }).catch(() => {});
        } catch {
          // Fallback to jsQR
        }
      }

      // 2. Pure JS canvas decode via jsQR
      try {
        const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
        const code = jsQR(imageData.data, imageData.width, imageData.height, {
          inversionAttempts: 'dontInvert',
        });

        if (code && code.data && !hasScannedRef.current) {
          if (code.data.startsWith('yntrapair://')) {
            hasScannedRef.current = true;
            stopCamera();
            onScan(code.data);
            return;
          }
        }
      } catch {
        // Next frame
      }
    }

    animationFrameRef.current = requestAnimationFrame(scanLoop);
  }, [onScan, stopCamera]);

  // Start selected camera stream
  const startCamera = useCallback(async (deviceId?: string) => {
    stopCamera();
    setError(null);
    hasScannedRef.current = false;
    setIsScanning(true);

    if (!navigator.mediaDevices?.getUserMedia) {
      setError(t('pairing.err_camera_unsupported') || 'Kamera stöds inte i denna miljö. Välj en bild istället.');
      return;
    }

    try {
      const constraints: MediaStreamConstraints = {
        video: deviceId
          ? { deviceId: { exact: deviceId } }
          : { facingMode: 'environment', width: { ideal: 1280 }, height: { ideal: 720 } },
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
    } catch (err: any) {
      const msg = err.name === 'NotAllowedError'
        ? (t('pairing.err_camera_permission') || 'Kamerabehörighet nekades. Tillåt kameraåtkomst eller välj en bild.')
        : (t('pairing.err_camera_init') || 'Kunde inte starta kameran. Kontrollera att ingen annan app använder den.');
      setError(msg);
    }
  }, [detectCameras, scanLoop, stopCamera, t]);

  useEffect(() => {
    if (isOpen) {
      startCamera(selectedCameraId || undefined);
    } else {
      stopCamera();
    }
    return () => {
      stopCamera();
    };
  }, [isOpen, selectedCameraId, startCamera, stopCamera]);

  // Switch camera
  const handleSwitchCamera = () => {
    if (cameras.length <= 1) return;
    const currentIndex = cameras.findIndex((c) => c.deviceId === selectedCameraId);
    const nextIndex = (currentIndex + 1) % cameras.length;
    const nextId = cameras[nextIndex].deviceId;
    setSelectedCameraId(nextId);
    startCamera(nextId);
  };

  // Handle file input upload (image file decoding)
  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    setError(null);
    const reader = new FileReader();
    reader.onload = (event) => {
      const img = new window.Image();
      img.onload = () => {
        const canvas = document.createElement('canvas');
        canvas.width = img.width;
        canvas.height = img.height;
        const ctx = canvas.getContext('2d');
        if (!ctx) return;

        ctx.drawImage(img, 0, 0);
        const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
        const code = jsQR(imageData.data, imageData.width, imageData.height);

        if (code && code.data && code.data.startsWith('yntrapair://')) {
          hasScannedRef.current = true;
          stopCamera();
          onScan(code.data);
        } else {
          setError(t('pairing.err_no_qr_found') || 'Hittade ingen giltig Yntra Vault QR-kod i den valda bilden.');
        }
      };
      img.src = event.target?.result as string;
    };
    reader.readAsDataURL(file);
    e.target.value = '';
  };

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
          >
            {/* Header */}
            <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border-subtle)] bg-[var(--bg-surface)]">
              <div className="flex items-center gap-2">
                <div className="h-7 w-7 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] flex items-center justify-center text-[var(--text-secondary)]">
                  <Camera className="w-3.5 h-3.5" />
                </div>
                <div>
                  <h3 className="text-sm font-semibold text-[var(--text-primary)]">
                    {t('pairing.scan_qr_title') || 'Skanna QR-kod'}
                  </h3>
                  <p className="text-[11px] text-[var(--text-muted)]">
                    {t('pairing.scan_qr_subtitle') || 'Rikta kameran mot QR-koden på datorn'}
                  </p>
                </div>
              </div>
              <button
                type="button"
                onClick={onClose}
                className="h-7 w-7 rounded-[3px] border border-transparent hover:border-[var(--border)] flex items-center justify-center text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors"
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

              {/* Viewfinder Target Frame */}
              <div className="relative z-10 w-[210px] h-[210px] pointer-events-none">
                {/* Corner Accents */}
                <div className="absolute top-0 left-0 w-7 h-7 border-t-2 border-l-2 border-white/90 rounded-tl-[3px]" />
                <div className="absolute top-0 right-0 w-7 h-7 border-t-2 border-r-2 border-white/90 rounded-tr-[3px]" />
                <div className="absolute bottom-0 left-0 w-7 h-7 border-b-2 border-l-2 border-white/90 rounded-bl-[3px]" />
                <div className="absolute bottom-0 right-0 w-7 h-7 border-b-2 border-r-2 border-white/90 rounded-br-[3px]" />

                {/* Animated Laser Scan Line */}
                {isScanning && !error && (
                  <motion.div
                    animate={{ y: [0, 204, 0] }}
                    transition={{ repeat: Infinity, duration: 2.2, ease: 'easeInOut' }}
                    className="w-full h-[1.5px] bg-emerald-400 shadow-[0_0_8px_rgba(52,211,153,0.8)]"
                  />
                )}
              </div>

              {/* Error Overlay */}
              {error && (
                <div className="absolute inset-0 z-20 flex flex-col items-center justify-center p-6 bg-black/85 text-center">
                  <AlertCircle className="w-8 h-8 text-red-400 mb-2" />
                  <p className="text-xs text-zinc-300 font-medium max-w-[280px] mb-4">
                    {error}
                  </p>
                  <button
                    type="button"
                    onClick={() => startCamera(selectedCameraId || undefined)}
                    className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-white bg-zinc-800 hover:bg-zinc-700 rounded-[3px] transition-colors"
                  >
                    <RefreshCw className="w-3.5 h-3.5" />
                    {t('common.retry') || 'Försök igen'}
                  </button>
                </div>
              )}
            </div>

            {/* Footer Actions */}
            <div className="flex items-center justify-between px-4 py-3 border-t border-[var(--border-subtle)] bg-[var(--bg-surface)]">
              {cameras.length > 1 ? (
                <button
                  type="button"
                  onClick={handleSwitchCamera}
                  className="flex items-center gap-1.5 px-2.5 py-1.5 text-xs font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] rounded-[3px] border border-[var(--border)] hover:bg-[var(--bg-elevated)] transition-colors"
                >
                  <RefreshCw className="w-3.5 h-3.5" />
                  {t('pairing.switch_camera') || 'Växla kamera'}
                </button>
              ) : (
                <div />
              )}

              {/* Upload Image Fallback */}
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
                  className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] rounded-[3px] border border-[var(--border)] hover:bg-[var(--bg-elevated)] transition-colors"
                >
                  <Image className="w-3.5 h-3.5" />
                  {t('pairing.pick_qr_image') || 'Välj bild med QR'}
                </button>
              </div>
            </div>
          </motion.div>
        </div>
      )}
    </AnimatePresence>
  );
};
