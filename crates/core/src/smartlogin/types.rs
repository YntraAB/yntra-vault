//! Shared types for the Smart Login engine.

use serde::{Serialize, Deserialize};

// ─── Page Snapshot Types ────────────────────────────────────────────────

/// Complete snapshot of a page's interactive elements, combining
/// DOM metadata and accessibility tree information.
#[derive(Debug, Clone, Serialize)]
pub struct PageSnapshot {
    pub url: String,
    pub title: String,
    pub forms: Vec<FormInfo>,
    pub inputs: Vec<InputInfo>,
    pub buttons: Vec<ButtonInfo>,
    pub links: Vec<LinkInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormInfo {
    pub form_id: Option<String>,
    pub action: String,
    pub method: String,
    pub input_count: usize,
    pub has_password_input: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputInfo {
    /// CDP backend node id for interaction
    pub backend_node_id: i64,
    pub input_type: String,
    pub name: String,
    pub id: String,
    pub placeholder: String,
    pub autocomplete: String,
    pub aria_label: String,
    pub associated_label: String,
    pub is_visible: bool,
    pub is_readonly: bool,
    pub form_index: Option<usize>,
    pub surrounding_text: String,
    pub ax_role: String,
    pub ax_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButtonInfo {
    pub backend_node_id: i64,
    pub text: String,
    pub button_type: String,
    pub aria_label: String,
    pub is_visible: bool,
    pub form_index: Option<usize>,
    pub ax_role: String,
    pub ax_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkInfo {
    pub backend_node_id: i64,
    pub text: String,
    pub href: String,
    pub aria_label: String,
    pub is_visible: bool,
    pub in_nav: bool,
    pub ax_role: String,
    pub ax_name: String,
}

// ─── Classification Types ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ClassifiedForm {
    pub identifier_field: Option<ScoredField>,
    pub password_field: Option<ScoredField>,
    pub submit_button: Option<ScoredButton>,
    pub continue_button: Option<ScoredButton>,
    pub method_selectors: Vec<ScoredButton>,
    pub identifier_type: IdentifierType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldRole {
    Username,
    Email,
    Phone,
    Password,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdentifierType {
    Username,
    Email,
    Phone,
    UsernameOrEmail,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ButtonRole {
    Login,
    Continue,
    MethodSelector,
    Other,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScoredField {
    pub input: InputInfo,
    pub confidence: f64,
    pub role: FieldRole,
    pub score_breakdown: Vec<(String, f64)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScoredButton {
    pub button: ButtonInfo,
    pub confidence: f64,
    pub role: ButtonRole,
    pub score_breakdown: Vec<(String, f64)>,
}

// ─── State Machine Types ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoginState {
    Idle,
    LaunchingBrowser,
    NavigatingToUrl,
    AnalyzingPage,
    SearchingForLogin,
    NavigatingToLogin,
    FormDetected,
    SelectingLoginMethod,
    FillingIdentifier,
    SubmittingIdentifier,
    WaitingForPasswordStep,
    FillingPassword,
    SubmittingLogin,
    VerifyingResult,
    Success,
    Failed(String),
    RequiresManualAction(ManualActionType),
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManualActionType {
    Captcha,
    TwoFactorAuth,
    AccountLocked,
    UnknownChallenge,
}

// ─── Result Types ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoginResult {
    Success { final_url: String },
    AlreadySignedIn { final_url: String },
    DifferentAccount,
    WrongCredentials { error_message: Option<String> },
    RequiresCaptcha,
    RequiresManualAction,
    RequiresMfa { mfa_type: String },
    AccountLocked { message: String },
    LoginFormNotFound,
    UnexpectedState { description: String },
    DomainMismatch { expected: String, actual: String },
    Timeout,
    BrowserError(String),
    Cancelled,
}

// ─── Configuration ──────────────────────────────────────────────────────

/// Runtime configuration for a Smart Login attempt.
#[derive(Debug, Clone)]
pub struct SmartLoginConfig {
    /// CDP remote debugging port
    pub cdp_port: u16,
    /// Max seconds to wait for page load
    pub page_load_timeout_secs: u64,
    /// Max seconds to wait for DOM stabilization after interaction
    pub dom_settle_timeout_secs: u64,
    /// Max state transitions before giving up
    pub max_state_transitions: usize,
    /// Max navigation attempts to find login form
    pub max_navigation_attempts: usize,
    /// Min confidence to accept a field classification
    pub min_field_confidence: f64,
    /// Min confidence to accept a button classification
    pub min_button_confidence: f64,
    /// Delay between keystrokes in ms (range for randomization)
    pub keystroke_delay_range_ms: (u64, u64),
}

impl Default for SmartLoginConfig {
    fn default() -> Self {
        Self {
            cdp_port: 0,
            page_load_timeout_secs: 15,
            dom_settle_timeout_secs: 5,
            max_state_transitions: 8,
            max_navigation_attempts: 3,
            min_field_confidence: 0.40,
            min_button_confidence: 0.35,
            keystroke_delay_range_ms: (15, 55),
        }
    }
}
