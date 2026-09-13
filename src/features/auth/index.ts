export { CreateVaultModal, default as CreateVaultModalDefault } from './components/CreateVaultModal';
export type { CreateVaultModalProps } from './components/CreateVaultModal';

export { ChangeMasterPasswordModal, default as ChangeMasterPasswordModalDefault } from './components/ChangeMasterPasswordModal';
export type { ChangeMasterPasswordModalProps } from './components/ChangeMasterPasswordModal';

export { Hardware2FaModal, default as Hardware2FaModalDefault } from './components/Hardware2FaModal';
export type { Hardware2FaModalProps } from './components/Hardware2FaModal';

export { AuthProvider, useAuth } from './context/AuthContext';
export type { AuthContextType } from './context/AuthContext';

export { useVault } from './hooks/useVault';
export { useBiometric } from './hooks/useBiometric';
export { useHardware2Fa } from './hooks/useHardware2Fa';
export { useAutoLock } from './hooks/useAutoLock';
export type { UseAutoLockOptions } from './hooks/useAutoLock';
