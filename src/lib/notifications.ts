import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification';

/**
 * Sends a native desktop notification if permissions are granted.
 * Gracefully falls back if running in browser, permission is denied, or Tauri is unavailable.
 */
export async function sendDesktopNotification(title: string, body?: string): Promise<void> {
  try {
    let granted = await isPermissionGranted();
    if (!granted) {
      const permission = await requestPermission();
      granted = permission === 'granted';
    }

    if (granted) {
      sendNotification({
        title,
        body,
      });
    }
  } catch {
    // Fail silently in non-desktop environments or when notifications are blocked
  }
}
