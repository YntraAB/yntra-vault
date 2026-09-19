import jsQR from 'jsqr';

export interface QrDecodeResult {
  success: boolean;
  text?: string;
  error?: 'NOT_FOUND' | 'INVALID_IMAGE' | 'EMPTY_FILE';
  errorMessage?: string;
}

/**
 * Loads an image File, Blob, or URL into an HTMLImageElement
 */
function loadImage(source: File | Blob | string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.crossOrigin = 'anonymous';

    const url = typeof source === 'string' ? source : URL.createObjectURL(source);

    img.onload = () => {
      if (typeof source !== 'string') {
        URL.revokeObjectURL(url);
      }
      resolve(img);
    };

    img.onerror = (err) => {
      if (typeof source !== 'string') {
        URL.revokeObjectURL(url);
      }
      reject(err);
    };

    img.src = url;
  });
}

interface DetectedBarcode {
  rawValue?: string;
  format?: string;
}

interface BarcodeDetectorInstance {
  detect: (source: ImageBitmap | HTMLImageElement | HTMLCanvasElement | HTMLVideoElement) => Promise<DetectedBarcode[]>;
}

declare global {
  interface Window {
    BarcodeDetector?: {
      new (options?: { formats: string[] }): BarcodeDetectorInstance;
    };
  }
}

/**
 * Attempts hardware-accelerated BarcodeDetector scan if available
 */
async function tryBarcodeDetector(source: ImageBitmap | HTMLImageElement | HTMLCanvasElement): Promise<string | null> {
  if (typeof window === 'undefined' || !window.BarcodeDetector) {
    return null;
  }

  try {
    const detector = new window.BarcodeDetector({ formats: ['qr_code'] });
    const barcodes = await detector.detect(source);
    if (barcodes && barcodes.length > 0 && barcodes[0].rawValue) {
      return barcodes[0].rawValue.trim();
    }
  } catch {
    // Unsupported or runtime error in BarcodeDetector
  }

  return null;
}

/**
 * Scans an HTMLImageElement at a specific max dimension scale using jsQR
 */
function scanImageAtScale(
  img: HTMLImageElement,
  maxDimension: number,
  applyContrast = false
): string | null {
  const origWidth = img.naturalWidth || img.width;
  const origHeight = img.naturalHeight || img.height;

  if (origWidth === 0 || origHeight === 0) return null;

  let width = origWidth;
  let height = origHeight;

  if (origWidth > maxDimension || origHeight > maxDimension) {
    if (origWidth >= origHeight) {
      width = maxDimension;
      height = Math.round((origHeight * maxDimension) / origWidth);
    } else {
      height = maxDimension;
      width = Math.round((origWidth * maxDimension) / origHeight);
    }
  }

  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  if (!ctx) return null;

  ctx.drawImage(img, 0, 0, width, height);
  const imageData = ctx.getImageData(0, 0, width, height);

  if (applyContrast) {
    // Apply contrast boost and thresholding to assist with screen glare or shadows
    const d = imageData.data;
    const factor = 1.4; // contrast multiplier
    for (let i = 0; i < d.length; i += 4) {
      // Gray value
      const gray = 0.299 * d[i] + 0.587 * d[i + 1] + 0.114 * d[i + 2];
      const contrast = Math.min(255, Math.max(0, factor * (gray - 128) + 128));
      d[i] = contrast;
      d[i + 1] = contrast;
      d[i + 2] = contrast;
    }
  }

  // Run jsQR with inversion attempts to handle both dark-on-light and light-on-dark QR codes
  const code = jsQR(imageData.data, width, height, {
    inversionAttempts: 'attemptBoth',
  });

  if (code && code.data && code.data.trim()) {
    return code.data.trim();
  }

  return null;
}

/**
 * Decodes a QR code from a File, Blob, Image, or Canvas with multi-scale downscaling,
 * native BarcodeDetector, and contrast fallback.
 */
export async function decodeQrFromImage(
  input: File | Blob | HTMLImageElement | string
): Promise<QrDecodeResult> {
  if (!input) {
    return { success: false, error: 'EMPTY_FILE', errorMessage: 'No image provided' };
  }

  try {
    // 1. Try native BarcodeDetector first on ImageBitmap (fastest & lowest memory)
    if (typeof window !== 'undefined' && 'BarcodeDetector' in window) {
      try {
        let bitmap: ImageBitmap | null = null;
        if (input instanceof File || input instanceof Blob) {
          bitmap = await createImageBitmap(input);
        } else if (input instanceof HTMLImageElement) {
          bitmap = await createImageBitmap(input);
        }
        if (bitmap) {
          const detectorResult = await tryBarcodeDetector(bitmap);
          bitmap.close?.();
          if (detectorResult) {
            return { success: true, text: detectorResult };
          }
        }
      } catch {
        // Continue to canvas fallback
      }
    }

    // 2. Load image element
    let img: HTMLImageElement;
    if (input instanceof HTMLImageElement) {
      img = input;
    } else {
      img = await loadImage(input);
    }

    const origWidth = img.naturalWidth || img.width;
    const origHeight = img.naturalHeight || img.height;

    if (origWidth === 0 || origHeight === 0) {
      return { success: false, error: 'INVALID_IMAGE', errorMessage: 'Could not load image dimensions' };
    }

    // 3. Multi-scale scanning strategy:
    // Phone photos are often 3000x4000 (12MP) or 48MP.
    // jsQR works best when the entire QR code is between 200px and 800px.
    // We try optimal resolutions in order of highest probability:
    const targetScales: number[] = [];

    const maxDim = Math.max(origWidth, origHeight);
    if (maxDim > 1200) {
      targetScales.push(1024); // Sweet spot for mobile camera photos
      targetScales.push(640);  // Great for larger QR codes in frame
      targetScales.push(1600); // For small QR codes in large high-res photos
      if (maxDim <= 2400) {
        targetScales.push(maxDim); // Full resolution if reasonable size
      }
    } else if (maxDim > 640) {
      targetScales.push(maxDim);
      targetScales.push(640);
      targetScales.push(1024);
    } else {
      targetScales.push(maxDim);
      targetScales.push(800);
    }

    // Attempt scan at each scale
    for (const scale of targetScales) {
      const text = scanImageAtScale(img, scale, false);
      if (text) {
        return { success: true, text };
      }
    }

    // 4. Fallback: Try with contrast enhancement on 1024px scale (helpful for photo of screens)
    const contrastText = scanImageAtScale(img, Math.min(maxDim, 1024), true);
    if (contrastText) {
      return { success: true, text: contrastText };
    }

    return {
      success: false,
      error: 'NOT_FOUND',
      errorMessage: 'No valid QR code was detected in the image',
    };
  } catch (err) {
    return {
      success: false,
      error: 'INVALID_IMAGE',
      errorMessage: err instanceof Error ? err.message : String(err),
    };
  }
}
