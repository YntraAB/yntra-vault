import type { ToastMessage } from '@/types';
import React,{ useCallback,useEffect,useMemo,useRef,useState } from 'react';
import { ToastContext } from './ToastContext.model';


export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<ToastMessage[]>([]);
  const timers = useRef(new Map<string, ReturnType<typeof setTimeout>>());
  const active = useRef(true);
  useEffect(() => {
    active.current = true;
    const pending = timers.current;
    return () => { active.current = false; pending.forEach(clearTimeout); pending.clear(); };
  }, []);

  const addToast = useCallback((msg: Omit<ToastMessage, 'id'>) => {
    if (!active.current) return;
    const id = Math.random().toString(36).slice(2);
    setToasts((prev) => [...prev, { ...msg, id }]);
    timers.current.set(id, setTimeout(() => {
      timers.current.delete(id);
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 3000));
  }, []);

  const removeToast = useCallback((id: string) => {
    clearTimeout(timers.current.get(id));
    timers.current.delete(id);
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  useEffect(() => {
    const failed = () => addToast({ type: 'error', message: 'App settings could not be saved. Check available storage before closing the app.' });
    window.addEventListener('yntra-metadata-save-failed', failed);
    return () => window.removeEventListener('yntra-metadata-save-failed', failed);
  }, [addToast]);

  const value = useMemo(() => ({ toasts, addToast, removeToast }), [toasts, addToast, removeToast]);

  return (
    <ToastContext.Provider value={value}>
      {children}
    </ToastContext.Provider>
  );
}
