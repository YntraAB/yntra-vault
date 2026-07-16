import { Routes, Route } from 'react-router-dom';
import { ThemeProvider } from '@/contexts/ThemeContext';
import { AppStateProvider } from '@/contexts/AppStateContext';
import VaultSelect from '@/pages/VaultSelect';
import Login from '@/pages/Login';
import AppLayout from '@/pages/AppLayout';

export default function App() {
  return (
    <ThemeProvider>
      <AppStateProvider>
        <Routes>
          <Route path="/" element={<VaultSelect />} />
          <Route path="/login" element={<Login />} />
          <Route path="/app" element={<AppLayout />} />
        </Routes>
      </AppStateProvider>
    </ThemeProvider>
  );
}



