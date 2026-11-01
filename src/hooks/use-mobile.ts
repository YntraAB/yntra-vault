import { useSyncExternalStore } from 'react';
import { useSettings } from '@/features/settings';

const MOBILE_QUERY = '(max-width: 767px)';
const TABLET_QUERY = '(min-width: 768px) and (max-width: 1023px)';
function subscribe(query: string, notify: () => void) {
  const media = window.matchMedia(query);
  media.addEventListener('change', notify);
  return () => media.removeEventListener('change', notify);
}
const subscribeMobile = (notify: () => void) => subscribe(MOBILE_QUERY, notify);
const subscribeTablet = (notify: () => void) => subscribe(TABLET_QUERY, notify);
const subscribeTouch = (notify: () => void) => subscribe('(any-pointer: coarse)', notify);
const noMatch = () => false;

export function useIsMobile() {
  return useSyncExternalStore(subscribeMobile, () => window.matchMedia(MOBILE_QUERY).matches, noMatch);
}
export function useMobile() {
  const isMobileScreen = useIsMobile();
  const { settings } = useSettings();
  const isTablet = useSyncExternalStore(subscribeTablet, () => window.matchMedia(TABLET_QUERY).matches, noMatch);
  const isTouchDevice = useSyncExternalStore(subscribeTouch, () => 'ontouchstart' in window || navigator.maxTouchPoints > 0, noMatch);
  return { isMobile: Boolean(settings?.forceMobileView || isMobileScreen), isTablet, isTouchDevice };
}
