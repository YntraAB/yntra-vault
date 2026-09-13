//! Structured event logging for Smart Login operations.
//! Credentials are never included in any event payload.

use serde::Serialize;
use crate::smartlogin::types::LoginState;

/// A single progress event emitted during a Smart Login attempt.
#[derive(Debug, Clone, Serialize)]
pub struct SmartLoginEvent {
    pub timestamp: String,
    pub state: LoginState,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<SmartLoginEventDetail>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum SmartLoginEventDetail {
    BrowserDetected {
        browser_path: String,
    },
    PageAnalysis {
        forms_found: usize,
        inputs_found: usize,
        buttons_found: usize,
        links_found: usize,
    },
    FieldScored {
        field_description: String,
        role: String,
        confidence: f64,
        breakdown: Vec<(String, f64)>,
    },
    LoginLinkFound {
        text: String,
        href: String,
        confidence: f64,
    },
    NavigatedTo {
        url: String,
    },
    Interaction {
        action: String,
        target_description: String,
    },
    Verification {
        result: String,
    },
    Error {
        reason: String,
    },
}

impl SmartLoginEvent {
    pub fn new(state: LoginState, message: impl Into<String>) -> Self {
        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            state,
            message: message.into(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: SmartLoginEventDetail) -> Self {
        self.detail = Some(detail);
        self
    }
}

/// Callback type for emitting events from the engine.
pub type EventCallback = Box<dyn Fn(SmartLoginEvent) + Send + Sync>;

/// Logger that wraps an event callback.
pub struct SmartLoginLogger {
    callback: Option<EventCallback>,
}

impl SmartLoginLogger {
    pub fn new(callback: EventCallback) -> Self {
        Self { callback: Some(callback) }
    }

    pub fn noop() -> Self {
        Self { callback: None }
    }

    pub fn emit(&self, event: SmartLoginEvent) {
        if let Some(cb) = &self.callback {
            cb(event);
        }
    }

    /// Shorthand for emitting a simple state+message event.
    pub fn log(&self, state: LoginState, message: impl Into<String>) {
        self.emit(SmartLoginEvent::new(state, message));
    }
}
