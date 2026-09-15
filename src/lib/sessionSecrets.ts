// In-memory transient secret storage for active session credentials (cleared on vault lock).
// Secrets are never written to disk, sessionStorage, or localStorage.

let transientWebdavPassword: string | null = null;

export function getTransientWebdavPassword(): string | null {
  return transientWebdavPassword;
}

export function setTransientWebdavPassword(password: string | null): void {
  transientWebdavPassword = password;
}

export function clearTransientWebdavPassword(): void {
  transientWebdavPassword = null;
}
