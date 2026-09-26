import { appMetadata, initializeAppMetadata } from '@/lib/appMetadata';
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { HashRouter } from 'react-router-dom'
import './index.css'
import App from './App.tsx'
import { ErrorBoundary } from '@/components/ui/ErrorBoundary'

// Catch unhandled errors and rejections to ensure visibility in logs
window.addEventListener('error', (event) => {
  console.error('Unhandled runtime error:', event.error || event.message);
});

window.addEventListener('unhandledrejection', (event) => {
  console.error('Unhandled promise rejection:', event.reason);
});

// Allow Shift + Right Click to always open the native WebView inspect element menu
window.addEventListener(
  'contextmenu',
  (e) => {
    if (e.shiftKey) {
      e.stopPropagation();
    }
  },
  true
);

async function start() {
  await initializeAppMetadata();
  // Restore metadata before any providers read settings or select a recent vault.
  if (!appMetadata.getItem('yntra-vault-setup-completed')) {
    appMetadata.setItem('yntra-vault-setup-completed', 'true');
  }
  createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ErrorBoundary>
      <HashRouter>
        <App />
      </HashRouter>
    </ErrorBoundary>
  </StrictMode>,
  );
}
void start().catch(() => {
  // Do not silently replace saved metadata with an empty first-run experience.
  const root = document.getElementById('root')!;
  const message = document.createElement('p');
  message.textContent = 'Saved app settings could not be loaded. Your vault files have not been changed.';
  const retry = document.createElement('button');
  retry.textContent = 'Try again';
  retry.onclick = () => window.location.reload();
  root.replaceChildren(message, retry);
});



