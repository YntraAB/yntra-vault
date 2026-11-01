
import { AuthProvider } from '@/features/auth';
import { EntriesProvider } from '@/features/entries';
import { SettingsProvider } from '@/features/settings';
import { ToastProvider } from './ToastContext';
import { UiProvider } from './UiContext';


// ─── Combined Provider Facade ───────────────────────────────────────────

export function AppStateProvider({ children }: { children: React.ReactNode }) {
  return (
    <SettingsProvider>
      <ToastProvider>
        <AuthProvider>
          <UiProvider>
            <EntriesProvider>
              {children}
            </EntriesProvider>
          </UiProvider>
        </AuthProvider>
      </ToastProvider>
    </SettingsProvider>
  );
}