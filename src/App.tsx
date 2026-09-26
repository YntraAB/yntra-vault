import { appMetadata } from '@/lib/appMetadata';
import { Routes, Route, Navigate } from 'react-router-dom';
import { ThemeProvider } from '@/contexts/ThemeContext';
import { AppStateProvider } from '@/contexts/AppStateContext';
import { LanguageProvider } from '@/contexts/LanguageContext';
import VaultSelect from '@/pages/VaultSelect';
import Login from '@/pages/Login';
import AppLayout from '@/pages/AppLayout';
import Onboarding from '@/pages/Onboarding';
import { UpdaterProvider } from '@/features/updater/useUpdater';

function RootRoute() {
  const setupCompleted = appMetadata.getItem('yntra-vault-setup-completed') === 'true';
  if (!setupCompleted) {
    return <Navigate to="/setup" replace />;
  }
  return <VaultSelect />;
}

export default function App() {
  return (
    <ThemeProvider>
      <AppStateProvider>
        <LanguageProvider>
          <UpdaterProvider>
          <Routes>
            <Route path="/" element={<RootRoute />} />
            <Route path="/setup" element={<Onboarding />} />
            <Route path="/login" element={<Login />} />
            <Route path="/app" element={<AppLayout />} />
            <Route path="*" element={<Navigate to="/" replace />} />
          </Routes>
          </UpdaterProvider>
        </LanguageProvider>
      </AppStateProvider>
    </ThemeProvider>
  );
}



