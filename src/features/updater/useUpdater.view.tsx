import { type ReactNode } from 'react';
import { UpdateModal } from './UpdateModal';
import { UpdaterContext,useUpdaterState } from './useUpdater.model';

export function UpdaterProvider({ children }: { children: ReactNode }) {
  const updater = useUpdaterState();
  return <UpdaterContext.Provider value={updater}>
    {children}
    <UpdateModal isOpen={updater.isModalOpen} onClose={() => updater.setIsModalOpen(false)}
      updateInfo={updater.updateInfo} currentVersion={updater.currentVersion}
      isDownloading={updater.isDownloading} onInstall={updater.installUpdate} error={updater.error} />
  </UpdaterContext.Provider>;
}