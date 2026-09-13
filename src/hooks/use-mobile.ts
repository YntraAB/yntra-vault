import * as React from "react";
import { useSettings } from '@/features/settings';

const MOBILE_BREAKPOINT = 768
const TABLET_BREAKPOINT = 1024

export function useIsMobile() {
  const [isMobile, setIsMobile] = React.useState<boolean>(() => {
    if (typeof window === 'undefined') return false;
    return window.innerWidth < MOBILE_BREAKPOINT;
  });

  React.useEffect(() => {
    const mql = window.matchMedia(`(max-width: ${MOBILE_BREAKPOINT - 1}px)`);
    const onChange = () => {
      setIsMobile(window.innerWidth < MOBILE_BREAKPOINT);
    };
    mql.addEventListener("change", onChange);
    setIsMobile(window.innerWidth < MOBILE_BREAKPOINT);
    return () => mql.removeEventListener("change", onChange);
  }, []);

  return isMobile;
}

export function useMobile() {
  const isMobileScreen = useIsMobile();
  const { settings } = useSettings();
  const [isTouch, setIsTouch] = React.useState<boolean>(false);
  const [isTablet, setIsTablet] = React.useState<boolean>(false);

  React.useEffect(() => {
    setIsTouch('ontouchstart' in window || navigator.maxTouchPoints > 0);
    const updateTablet = () => {
      const w = window.innerWidth;
      setIsTablet(w >= MOBILE_BREAKPOINT && w < TABLET_BREAKPOINT);
    };
    updateTablet();
    window.addEventListener('resize', updateTablet);
    return () => window.removeEventListener('resize', updateTablet);
  }, []);

  return {
    isMobile: Boolean(settings?.forceMobileView || isMobileScreen),
    isTablet,
    isTouchDevice: isTouch,
  };
}




