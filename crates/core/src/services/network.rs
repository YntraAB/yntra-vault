//! Process-wide policy for optional network features.
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use crate::{Result, VaultError};

struct Policy {
    state: AtomicU64,
    changed: tokio::sync::Notify,
}

impl Policy {
    const fn new(enabled: bool) -> Self {
        Self { state: AtomicU64::new(enabled as u64), changed: tokio::sync::Notify::const_new() }
    }

    fn set(&self, enabled: bool) {
        self.state.fetch_update(Ordering::AcqRel, Ordering::Acquire, |state| {
            if (state & 1 != 0) == enabled { None }
            else { Some((state.wrapping_add(2) & !1) | enabled as u64) }
        }).ok();
        self.changed.notify_waiters();
    }

    fn ensure(&self) -> Result<()> {
        if self.state.load(Ordering::Acquire) & 1 != 0 { Ok(()) }
        else { Err(VaultError::InvalidState("Network access is disabled in Closed System mode".into())) }
    }

    async fn run<T>(&self, operation: impl Future<Output = Result<T>>) -> Result<T> {
        let changed = self.changed.notified();
        tokio::pin!(changed);
        changed.as_mut().enable();
        let generation = self.state.load(Ordering::Acquire);
        if generation & 1 == 0 {
            return Err(VaultError::InvalidState("Network access is disabled in Closed System mode".into()));
        }
        tokio::pin!(operation);
        loop {
            tokio::select! {
                biased;
                _ = &mut changed => {
                    if self.state.load(Ordering::Acquire) != generation {
                        return Err(VaultError::InvalidState("Network operation cancelled by privacy settings".into()));
                    }
                    changed.set(self.changed.notified());
                    changed.as_mut().enable();
                }
                result = &mut operation => {
                    if self.state.load(Ordering::Acquire) != generation {
                        return Err(VaultError::InvalidState("Network operation cancelled by privacy settings".into()));
                    }
                    return result;
                }
            }
        }
    }
}

// CLI network commands are explicit. The desktop/mobile host denies access
// before startup and enables it only after loading the user's preferences.
static POLICY: Policy = Policy::new(true);

pub fn set_enabled(enabled: bool) { POLICY.set(enabled); }
pub fn ensure_allowed() -> Result<()> { POLICY.ensure() }
pub fn is_enabled() -> bool { POLICY.ensure().is_ok() }
pub fn generation_token() -> Result<u64> {
    let token = POLICY.state.load(Ordering::Acquire);
    if token & 1 == 0 { return Err(VaultError::InvalidState("Network access is disabled in Closed System mode".into())); }
    Ok(token)
}
pub fn ensure_generation(token: u64) -> Result<()> {
    if token & 1 == 0 || POLICY.state.load(Ordering::Acquire) != token {
        return Err(VaultError::InvalidState("Network operation cancelled by privacy settings".into()));
    }
    Ok(())
}
pub async fn run<T>(operation: impl Future<Output = Result<T>>) -> Result<T> { POLICY.run(operation).await }

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn denied_operations_are_never_polled_and_inflight_work_is_cancelled() {
        let policy = Policy::new(false);
        assert!(policy.run(async { panic!("denied operation was polled"); #[allow(unreachable_code)] Ok(()) }).await.is_err());
        policy.set(true);
        let work = policy.run(std::future::pending::<Result<()>>());
        let deny = async { tokio::task::yield_now().await; policy.set(false); };
        let (result, _) = tokio::join!(work, deny);
        assert!(result.is_err());
        policy.set(true);
        assert_eq!(policy.run(async { Ok(42) }).await.unwrap(), 42);
    }
}
