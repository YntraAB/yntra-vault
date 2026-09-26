import { useCallback, useEffect, useRef } from 'react';
import type { MouseEvent as ReactMouseEvent } from 'react';

export function panelWidths(sidebar: number, list: number, viewport: number) {
  const width = Math.max(768, Number.isFinite(viewport) ? viewport : 1024);
  const sidebarMax = Math.min(350, width - 260 - 220);
  const safeSidebar = Math.min(sidebarMax, Math.max(180, Number.isFinite(sidebar) ? sidebar : 220));
  return {
    sidebarWidth: safeSidebar,
    passwordListWidth: Math.min(450, width - safeSidebar - 260, Math.max(220, Number.isFinite(list) ? list : 320)),
  };
}

export function usePanelResize(sidebarWidth: number, passwordListWidth: number, save: (widths: { sidebarWidth: number; passwordListWidth: number }) => void) {
  const widths = useRef({ sidebarWidth, passwordListWidth });
  const drag = useRef<{ target: 'sidebar' | 'list'; x: number; width: number; userSelect: string } | null>(null);
  const apply = useCallback(() => {
    widths.current = panelWidths(widths.current.sidebarWidth, widths.current.passwordListWidth, window.innerWidth);
    document.documentElement.style.setProperty('--sidebar-width', `${widths.current.sidebarWidth}px`);
    document.documentElement.style.setProperty('--passwordlist-width', `${widths.current.passwordListWidth}px`);
  }, []);

  useEffect(() => {
    if (!drag.current) { widths.current = { sidebarWidth, passwordListWidth }; apply(); }
  }, [sidebarWidth, passwordListWidth, apply]);

  useEffect(() => {
    const move = (event: MouseEvent) => {
      if (!drag.current) return;
      const { target, x, width } = drag.current;
      const value = width + event.clientX - x;
      if (target === 'sidebar') widths.current.sidebarWidth = value;
      else widths.current.passwordListWidth = value;
      apply();
    };
    const finish = () => {
      if (!drag.current) return;
      document.body.style.userSelect = drag.current.userSelect;
      drag.current = null;
      save({ ...widths.current });
    };
    const resize = () => { finish(); apply(); };
    document.addEventListener('mousemove', move);
    document.addEventListener('mouseup', finish);
    window.addEventListener('blur', finish);
    window.addEventListener('resize', resize);
    return () => {
      if (drag.current) document.body.style.userSelect = drag.current.userSelect;
      drag.current = null;
      document.removeEventListener('mousemove', move);
      document.removeEventListener('mouseup', finish);
      window.removeEventListener('blur', finish);
      window.removeEventListener('resize', resize);
    };
  }, [apply, save]);

  const start = useCallback((target: 'sidebar' | 'list', event: ReactMouseEvent) => {
    if (event.button !== 0) return;
    drag.current = { target, x: event.clientX, width: target === 'sidebar' ? widths.current.sidebarWidth : widths.current.passwordListWidth, userSelect: document.body.style.userSelect };
    document.body.style.userSelect = 'none';
    event.preventDefault();
  }, []);
  return {
    handleSidebarResizeStart: useCallback((event: ReactMouseEvent) => start('sidebar', event), [start]),
    handleListResizeStart: useCallback((event: ReactMouseEvent) => start('list', event), [start]),
  };
}
