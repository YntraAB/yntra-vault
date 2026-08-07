import { Routes, Route, Navigate } from 'react-router-dom';
import { ThemeProvider } from '@/contexts/ThemeContext';
import { AppStateProvider } from '@/contexts/AppStateContext';
import { LanguageProvider } from '@/contexts/LanguageContext';
import VaultSelect from '@/pages/VaultSelect';
import Login from '@/pages/Login';
import AppLayout from '@/pages/AppLayout';
import Onboarding from '@/pages/Onboarding';

function RootRoute() {
  const setupCompleted = localStorage.getItem('yntra-vault-setup-completed') === 'true';
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
          <Routes>
            <Route path="/" element={<RootRoute />} />
            <Route path="/setup" element={<Onboarding />} />
            <Route path="/login" element={<Login />} />
            <Route path="/app" element={<AppLayout />} />
          </Routes>
        </LanguageProvider>
      </AppStateProvider>
    </ThemeProvider>
  );
}



