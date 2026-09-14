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

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ErrorBoundary>
      <HashRouter>
        <App />
      </HashRouter>
    </ErrorBoundary>
  </StrictMode>,
)



