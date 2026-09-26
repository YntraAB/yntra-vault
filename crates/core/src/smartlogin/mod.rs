pub mod types;
pub mod browser;
pub mod analyzer;
pub mod discovery;
pub mod classifier;
pub mod engine;
pub mod verifier;
pub mod logging;
pub(crate) mod outcome;
#[cfg(any(target_os = "windows", test))]
pub(crate) mod native_state;

pub use types::*;
pub use engine::SmartLoginEngine;
pub use logging::SmartLoginEvent;
pub use browser::{BrowserInfo, PreCheckResult};
