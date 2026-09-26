//! Interaction Engine — the core state machine that orchestrates
//! browser launch, page analysis, field classification, credential filling,
//! multi-step flow handling, and result verification.

use chromiumoxide::page::Page;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use zeroize::Zeroizing;

use crate::smartlogin::types::*;
use crate::smartlogin::logging::*;
use crate::smartlogin::{browser, analyzer, discovery, verifier};

#[cfg(all(test, target_os = "windows"))]
#[path = "keyboard_windows_tests.rs"]
mod keyboard_windows_tests;

/// Google rejects some remotely controlled browser sessions before password entry.
pub fn uses_native_browser(url: &str) -> bool {
    if !cfg!(target_os = "windows") { return false; }
    let normalized = if url.contains("://") { url.to_owned() } else { format!("https://{url}") };
    reqwest::Url::parse(&normalized).is_ok_and(|u| {
        u.scheme() == "https" && u.username().is_empty() && u.password().is_none()
            && u.host_str().is_some_and(|host| ["google.com", "gmail.com"].iter()
                .any(|base| host == *base || host.ends_with(&format!(".{base}"))))
    })
}

#[cfg(all(test, target_os = "windows"))]
mod native_routing_tests {
    use super::uses_native_browser;
    #[test]
    fn google_uses_ordinary_browser_with_strict_host_boundaries() {
        for url in ["https://gmail.com", "https://mail.google.com/mail", "https://accounts.google.com/signin", "gmail.com"] {
            assert!(uses_native_browser(url));
        }
        for url in ["https://google.com.evil.test", "https://notgoogle.com", "https://google.com@evil.test", "http://gmail.com", "https://github.com", "file:///google.com"] {
            assert!(!uses_native_browser(url));
        }
    }
}

/// Top-level facade for executing a Smart Login attempt.
pub struct SmartLoginEngine {
    config: SmartLoginConfig,
    logger: Arc<SmartLoginLogger>,
    cancel_flag: Arc<AtomicBool>,
}

impl SmartLoginEngine {
    pub fn new(
        config: SmartLoginConfig,
        logger: SmartLoginLogger,
        cancel_flag: Arc<AtomicBool>,
    ) -> Self {
        Self {
            config,
            logger: Arc::new(logger),
            cancel_flag,
        }
    }

    /// Execute the full Smart Login flow for a given entry.
    /// Credentials are consumed and zeroed after use.
    pub async fn execute(
        &self,
        url: &str,
        identifier: Zeroizing<String>,
        password: Zeroizing<String>,
        totp_secret: Option<Zeroizing<String>>,
        browser_info: &browser::BrowserInfo,
    ) -> LoginResult {
        self.logger.log(LoginState::Idle, "Starting Smart Login for the saved website");

        #[cfg(target_os = "windows")]
        if uses_native_browser(url) {
            let browser = browser_info.clone();
            let logger = Arc::clone(&self.logger);
            let cancel = Arc::clone(&self.cancel_flag);
            let entry_url = url.to_owned();
            return tokio::task::spawn_blocking(move || {
                crate::services::autotype::run_native_google_login(&browser, &entry_url, identifier, password, &cancel, &logger)
            }).await.unwrap_or_else(|_| LoginResult::BrowserError("Keyboard login worker failed".into()));
        }

        // Phase 1: Launch browser and navigate
        let (session, page) = match browser::launch_and_connect(
            url,
            browser_info,
            &self.config,
            &self.logger,
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                return LoginResult::BrowserError(format!("Browser launch failed: {e}"));
            }
        };

        if self.is_cancelled() {
            return LoginResult::Cancelled;
        }

        // Check for an existing, identity-confirmed session before opening another login form.
        let _ = analyzer::wait_for_page_ready(&page, 8000).await;
        if let Some(result) = verifier::existing_session(&page, url, &identifier).await {
            if self.is_cancelled() { return LoginResult::Cancelled; }
            self.logger.log(LoginState::VerifyingResult, "An authenticated session was detected; skipping login-form discovery");
            session.disconnect();
            return result;
        }

        // Phase 2: Analyze page and find/navigate to login form
        let page = match self.find_login_form(page, url, &identifier).await {
            Ok(p) => p,
            Err(result) => return result,
        };

        if self.is_cancelled() {
            return LoginResult::Cancelled;
        }

        // Phase 3: Fill credentials and submit
        let result = self
            .fill_and_submit(&page, url, &identifier, password)
            .await;

        // Phase 4: Fill TOTP only after the verifier explicitly identifies MFA.
        let result = match (&result, &totp_secret) {
            (LoginResult::RequiresMfa { .. }, Some(secret)) => {
                self.handle_totp(&page, url, &identifier, secret).await
            }
            _ => result,
        };

        // Disconnect CDP so the browser resumes normal operation
        session.disconnect();

        result
    }

    /// Generate and fill a TOTP code when 2FA is required.
    async fn handle_totp(
        &self,
        page: &Page,
        entry_url: &str,
        identifier: &str,
        totp_secret: &Zeroizing<String>,
    ) -> LoginResult {
        self.logger.log(LoginState::FillingIdentifier, "2FA detected — generating TOTP code...");

        // Parse TOTP config (supports both otpauth:// URIs and raw base32 secrets)
        let secret_str = totp_secret.as_str().trim();
        let config = if secret_str.starts_with("otpauth://") {
            match crate::totp::parse_otpauth_uri(secret_str) {
                Ok(cfg) => cfg,
                Err(e) => {
                    self.logger.log(
                        LoginState::Failed(format!("Invalid otpauth URI: {e}")),
                        "Could not parse TOTP URI",
                    );
                    return LoginResult::RequiresMfa {
                        mfa_type: "Invalid TOTP URI format — enter code manually".into(),
                    };
                }
            }
        } else {
            crate::totp::TotpConfig {
                secret: secret_str.to_string(),
                ..Default::default()
            }
        };

        let totp_code = match crate::totp::generate_totp(&config) {
            Ok(code) => Zeroizing::new(code.code),
            Err(e) => {
                self.logger.log(
                    LoginState::Failed(format!("TOTP generation failed: {e}")),
                    "Could not generate TOTP code",
                );
                return LoginResult::RequiresMfa {
                    mfa_type: "TOTP generation failed — enter code manually".into(),
                };
            }
        };

        // Wait for OTP input to appear
        analyzer::wait_for_page_ready(page, 5000).await;

        // Fill the OTP field
        let fill_result = self.direct_fill_otp(page, &totp_code).await;
        if let Err(e) = fill_result {
            self.logger.log(
                LoginState::Failed(format!("OTP fill failed: {e}")),
                "Could not fill TOTP field — enter code manually",
            );
            return LoginResult::RequiresMfa {
                mfa_type: "Could not find OTP field — enter code manually".into(),
            };
        }

        self.logger.log(LoginState::SubmittingLogin, "Submitting TOTP code...");
        self.direct_submit(page).await;

        let snapshot_url = page.evaluate("window.location.href").await
            .ok()
            .and_then(|v| v.into_value::<String>().ok())
            .unwrap_or_default();

        match verifier::verify_login_result(page, &snapshot_url, entry_url, identifier, true, &self.cancel_flag, &self.logger).await {
            Ok(result) => result,
            Err(e) => LoginResult::BrowserError(format!("Post-TOTP verification failed: {e}")),
        }
    }

    /// Fill a TOTP/OTP input field visually (simulating human typing).
    async fn direct_fill_otp(
        &self,
        page: &Page,
        code: &Zeroizing<String>,
    ) -> crate::Result<()> {
        let focus_js = r#"
        (() => {
            // Strategy 1: Multi-box input (e.g. 6 separate inputs)
            const singleCharInputs = Array.from(document.querySelectorAll('input')).filter(el => 
                el.offsetParent !== null && 
                (el.maxLength === 1 || el.getAttribute('maxlength') === '1') &&
                (el.type === 'text' || el.type === 'tel' || el.type === 'number')
            );
            
            if (singleCharInputs.length >= 4) {
                singleCharInputs[0].focus();
                singleCharInputs[0].click();
                return 'found';
            }

            // Strategy 2: Standard single-box input
            const selectors = [
                'input[autocomplete="one-time-code"]',
                'input[name*="otp" i]', 'input[name*="totp" i]', 'input[name*="mfa" i]',
                'input[name*="2fa" i]', 'input[name*="code" i]', 'input[name*="pin" i]',
                'input[name*="otc" i]', 'input[name*="two_factor" i]', 'input[name*="app_totp" i]',
                'input[id*="otp" i]', 'input[id*="totp" i]', 'input[id*="mfa" i]',
                'input[id*="2fa" i]', 'input[id*="code" i]', 'input[id*="pin" i]',
                'input[id*="otc" i]', 'input[id*="app_totp" i]', 'input[inputmode="numeric"]',
                'input[type="tel"][maxlength="6"]', 'input[type="number"][maxlength="6"]',
                'input[type="tel"][maxlength="8"]', 'input[type="number"][maxlength="8"]',
            ];
            
            for (const sel of selectors) {
                const el = document.querySelector(sel);
                if (el && el.offsetParent !== null) {
                    el.focus();
                    el.click();
                    return 'found';
                }
            }

            // Strategy 3: Fallback any short text input
            const inputs = document.querySelectorAll('input[type="text"], input[type="tel"], input[type="number"]');
            for (const el of inputs) {
                if (el.offsetParent !== null && !el.value && ((el.maxLength > 0 && el.maxLength <= 8) || el.getAttribute('inputmode') === 'numeric')) {
                    el.focus();
                    el.click();
                    return 'found';
                }
            }
            
            return 'not_found';
        })()
        "#;

        let result = page.evaluate(focus_js).await.map_err(|e| {
            crate::error::VaultError::SmartLoginError(format!("OTP field focus failed: {e}"))
        })?;
        
        let status: String = result.into_value().unwrap_or_else(|_| "error".into());
        if status != "found" {
            return Err(crate::error::VaultError::SmartLoginError(
                "No OTP input field found on page".into(),
            ));
        }

        // Type character by character into document.activeElement using JS 
        // to avoid Chromium CDP "Lost Focus" pseudo-class issues when the site auto-advances.
        for ch in code.chars() {
            let ch_json = serde_json::to_string(&ch.to_string()).unwrap_or_else(|_| "\"\"".into());
            let type_js = format!(
                r#"
                (() => {{
                    const el = document.activeElement;
                    if (el && el.tagName === 'INPUT') {{
                        const val = {ch_json};
                        // Append character to value if it's a single box, or set it if it's a multi-box
                        if (el.maxLength === 1) {{
                            el.value = val;
                        }} else {{
                            el.value += val;
                        }}
                        el.dispatchEvent(new Event('input', {{bubbles: true, composed: true}}));
                        
                        // Fire a simulated keydown/keyup so React/Vue auto-advance scripts trigger!
                        el.dispatchEvent(new KeyboardEvent('keydown', {{key: val, bubbles: true}}));
                        el.dispatchEvent(new KeyboardEvent('keyup', {{key: val, bubbles: true}}));
                    }}
                }})()
                "#
            );
            let _ = page.evaluate(type_js).await;
            tokio::time::sleep(tokio::time::Duration::from_millis(60)).await;
        }
        
        // Final change/blur dispatch
        let _ = page.evaluate(r#"
            (() => {
                const el = document.activeElement;
                if (el && el.tagName === 'INPUT') {
                    el.dispatchEvent(new Event('change', {bubbles: true, composed: true}));
                    el.dispatchEvent(new Event('blur', {bubbles: true, composed: true}));
                }
            })()
        "#).await;

        self.logger.log(LoginState::FillingIdentifier, "TOTP code filled [REDACTED]");
        Ok(())
    }

    /// Analyze the page, find the login form (navigating if needed).
    /// Tries common login paths and handles cross-domain auth flows.
    async fn find_login_form(&self, page: Page, entry_url: &str, identifier: &str) -> Result<Page, LoginResult> {
        let current_page = page;
        let mut nav_attempts = 0;
        let mut visited = std::collections::HashSet::new();

        loop {
            if self.is_cancelled() {
                return Err(LoginResult::Cancelled);
            }

            // Wait for page to have interactive content before analyzing
            analyzer::wait_for_page_ready(&current_page, 8000).await;
            if self.is_cancelled() { return Err(LoginResult::Cancelled); }
            if let Some(result) = verifier::existing_session(&current_page, entry_url, identifier).await {
                self.logger.log(LoginState::VerifyingResult, "An authenticated session was detected; stopping login-form discovery");
                return Err(if self.is_cancelled() { LoginResult::Cancelled } else { result });
            }

            // Analyze current page
            let snapshot = match analyzer::analyze_page(&current_page, &self.logger).await {
                Ok(s) => s,
                Err(e) => {
                    return Err(LoginResult::BrowserError(format!(
                        "Page analysis failed: {e}"
                    )));
                }
            };

            // Check if login form is already here
            if discovery::has_login_form(&snapshot) {
                self.logger
                    .log(LoginState::FormDetected, "Login form detected on page");
                return Ok(current_page);
            }

            // Try login link candidates
            visited.insert(snapshot.url.clone());
            let candidates: Vec<_> = discovery::find_login_links(&snapshot, &self.logger).into_iter()
                .filter(|candidate| candidate.link.href.is_empty()
                    || (discovery::login_target_allowed(entry_url, &candidate.link.href)
                        && !visited.contains(&candidate.link.href))).collect();

            if !candidates.is_empty() && nav_attempts < self.config.max_navigation_attempts {
                let best = &candidates[0];

                // Navigate to the best candidate
                if !best.link.href.is_empty() {
                    // Verify domain is allowed (supports cross-domain auth like Gmail → accounts.google.com)
                    if !discovery::is_allowed_auth_domain(entry_url, &best.link.href) {
                        self.logger.log(
                            LoginState::SearchingForLogin,
                            format!("Skipping cross-domain link: {}", best.link.href),
                        );
                    } else {
                        self.logger.emit(
                            SmartLoginEvent::new(
                                LoginState::NavigatingToLogin,
                                format!("Navigating to: \"{}\"", best.link.text),
                            )
                            .with_detail(SmartLoginEventDetail::NavigatedTo {
                                url: best.link.href.clone(),
                            }),
                        );

                        visited.insert(best.link.href.clone());
                        match current_page.goto(&best.link.href).await {
                            Ok(_) => {}
                            Err(e) => {
                                return Err(LoginResult::BrowserError(format!(
                                    "Navigation failed: {e}"
                                )));
                            }
                        }

                        analyzer::wait_for_page_ready(&current_page, 8000).await;

                        nav_attempts += 1;
                        if nav_attempts <= self.config.max_navigation_attempts {
                            continue;
                        }
                    }
                } else {
                    // It's a button — click it
                    self.logger.emit(
                        SmartLoginEvent::new(
                            LoginState::NavigatingToLogin,
                            format!("Clicking: \"{}\"", best.link.text),
                        )
                        .with_detail(SmartLoginEventDetail::NavigatedTo {
                            url: String::new(),
                        }),
                    );

                    let search_target_json = serde_json::to_string(&best.link.text.trim().to_lowercase())
                        .unwrap_or_else(|_| "\"\"".to_string());
                    let click_js = format!(
                        r#"
                        (() => {{
                            const target = {};
                            const els = document.querySelectorAll('button, [role="button"], a, [role="link"]');
                            for (const el of els) {{
                                const text = (el.textContent || '').trim().toLowerCase();
                                if (text === target) {{
                                    el.click();
                                    return 'clicked';
                                }}
                            }}
                            return 'not_found';
                        }})()
                        "#,
                        search_target_json
                    );
                    let _ = current_page.evaluate(click_js).await;

                    analyzer::wait_for_page_ready(&current_page, 8000).await;

                    nav_attempts += 1;
                    if nav_attempts <= self.config.max_navigation_attempts {
                        continue;
                    }
                }
            }

            // No candidates found — try probing common login paths
            {
                let probe_urls = discovery::get_probe_urls(entry_url);

                for probe_url in &probe_urls {
                    if self.is_cancelled() { return Err(LoginResult::Cancelled); }
                    if !visited.insert(probe_url.clone()) { continue; }
                    self.logger.log(
                        LoginState::SearchingForLogin,
                        format!("Probing: {probe_url}"),
                    );

                    match current_page.goto(probe_url).await {
                        Ok(_) => {
                            // Wait for page to fully render (handles redirect chains)
                            self.logger.log(
                                LoginState::SearchingForLogin,
                                "Waiting for page to load...",
                            );
                            analyzer::wait_for_page_ready(&current_page, 8000).await;
                            if self.is_cancelled() { return Err(LoginResult::Cancelled); }
                            if let Some(result) = verifier::existing_session(&current_page, entry_url, identifier).await {
                                self.logger.log(LoginState::VerifyingResult, "Login navigation returned an authenticated session; stopping discovery");
                                return Err(if self.is_cancelled() { LoginResult::Cancelled } else { result });
                            }

                            // Re-analyze after probe navigation
                            if let Ok(snap) = analyzer::analyze_page(&current_page, &self.logger).await
                                && discovery::has_login_form(&snap) {
                                    self.logger.log(
                                        LoginState::FormDetected,
                                        format!("Login form found at {probe_url}"),
                                    );
                                    return Ok(current_page);
                                }
                        }
                        Err(_) => continue,
                    }
                }
            }

            // Exhausted all options
            self.logger.log(
                LoginState::Failed("Login form not found".into()),
                "Could not find a login form on any probed page",
            );
            return Err(LoginResult::LoginFormNotFound);
        }
    }

    /// Classify form fields, fill credentials, and submit.
    /// Uses classifier first, falls back to direct JS fill (Bitwarden-style).
    async fn fill_and_submit(
        &self,
        page: &Page,
        entry_url: &str,
        identifier: &Zeroizing<String>,
        password: Zeroizing<String>,
    ) -> LoginResult {
        let mut transitions = 0;
        let mut account_chooser_clicks = 0;

        loop {
            if self.is_cancelled() {
                return LoginResult::Cancelled;
            }

            transitions += 1;
            if transitions > self.config.max_state_transitions {
                return LoginResult::UnexpectedState {
                    description: "Too many state transitions — possible loop".into(),
                };
            }

            // Wait for page content
            analyzer::wait_for_page_ready(page, 5000).await;

            // Re-analyze the page
            let snapshot = match analyzer::analyze_page(page, &self.logger).await {
                Ok(s) => s,
                Err(e) => return LoginResult::BrowserError(format!("Analysis failed: {e}")),
            };

            // Verify domain is allowed before filling anything
            if !discovery::is_allowed_auth_domain(entry_url, &snapshot.url) {
                return LoginResult::DomainMismatch {
                    expected: entry_url.into(),
                    actual: snapshot.url,
                };
            }

            // Detect which fields exist on this page
            let has_password_input = snapshot.inputs.iter().any(|i| i.is_visible && !i.is_readonly && i.input_type == "password");
            let has_identifier_input = snapshot.inputs.iter().any(|i| {
                i.is_visible && !i.is_readonly && (matches!(i.input_type.as_str(), "email" | "tel")
                    || (i.input_type == "text" && !self.is_search_field(i)))
            });

            // Single-step form: both identifier AND password on same page
            if has_identifier_input && has_password_input {
                self.logger.log(LoginState::FillingIdentifier, "Filling identifier...");
                let fill_result = self.direct_fill_field(page, "identifier", &identifier).await;
                if let Err(e) = fill_result {
                    return LoginResult::BrowserError(format!("Identifier fill failed: {e}"));
                }

                self.logger.log(LoginState::FillingPassword, "Filling password...");
                let fill_result = self.direct_fill_field(page, "password", &password).await;
                if let Err(e) = fill_result {
                    return LoginResult::BrowserError(format!("Password fill failed: {e}"));
                }

                self.logger.log(LoginState::SubmittingLogin, "Submitting...");
                self.direct_submit(page).await;

                return match verifier::verify_login_result(page, &snapshot.url, entry_url, identifier, false, &self.cancel_flag, &self.logger).await {
                    Ok(result) => result,
                    Err(e) => LoginResult::BrowserError(format!("Verification failed: {e}")),
                };
            }

            // Multi-step second page: password only (identifier already submitted)
            if has_password_input {
                self.logger.log(LoginState::FillingPassword, "Filling password...");
                let fill_result = self.direct_fill_field(page, "password", &password).await;
                if let Err(e) = fill_result {
                    return LoginResult::BrowserError(format!("Password fill failed: {e}"));
                }

                self.logger.log(LoginState::SubmittingLogin, "Submitting...");
                self.direct_submit(page).await;

                return match verifier::verify_login_result(page, &snapshot.url, entry_url, identifier, false, &self.cancel_flag, &self.logger).await {
                    Ok(result) => result,
                    Err(e) => LoginResult::BrowserError(format!("Verification failed: {e}")),
                };
            }

            // Multi-step first page: identifier only → click Next
            if has_identifier_input {
                self.logger.log(LoginState::FillingIdentifier, "Filling identifier...");
                let fill_result = self.direct_fill_field(page, "identifier", &identifier).await;
                if let Err(e) = fill_result {
                    return LoginResult::BrowserError(format!("Identifier fill failed: {e}"));
                }

                self.logger.log(LoginState::SubmittingIdentifier, "Clicking Next...");
                self.direct_submit(page).await;

                self.logger.log(LoginState::WaitingForPasswordStep, "Waiting for password step...");
                tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
                continue;
            }

            // No identifier or password field found — try account chooser
            if account_chooser_clicks < 2 {
                let clicked = self.try_account_chooser(page).await;
                if clicked {
                    account_chooser_clicks += 1;
                    self.logger.log(
                        LoginState::NavigatingToLogin,
                        format!("Account chooser clicked ({account_chooser_clicks}/2)"),
                    );
                    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                    continue;
                }
            }

            // Nothing worked
            self.logger.log(
                LoginState::Failed("No fillable fields found".into()),
                "Could not find identifier or password fields on this page",
            );
            return LoginResult::LoginFormNotFound;
        }
    }

    /// Check if an input is likely a search field (not a login field).
    fn is_search_field(&self, input: &InputInfo) -> bool {
        let all = format!(
            "{} {} {} {} {}",
            input.name, input.id, input.placeholder, input.aria_label, input.autocomplete
        ).to_lowercase();
        all.contains("search") || all.contains("query") || all.contains("q")
            || input.input_type == "search"
    }

    /// Fill a field using direct JS injection with Bitwarden-style event dispatch.
    /// `field_type` is "identifier" or "password".
    async fn direct_fill_field(
        &self,
        page: &Page,
        field_type: &str,
        value: &Zeroizing<String>,
    ) -> crate::Result<()> {
        // A hidden password input must never leave the identifier focused while
        // the engine proceeds to type a password (Google includes such a decoy).
        let find_js = format!("({})({})", include_str!("select_field.js"), serde_json::to_string(field_type).unwrap());
        let result = page.evaluate(find_js).await.map_err(|_| {
            crate::error::VaultError::SmartLoginError("Could not select the credential field".into())
        })?;
        let status: String = result.into_value().unwrap_or_default();
        if status != "found" {
            return Err(crate::error::VaultError::SmartLoginError(format!("No editable {field_type} field found on page")));
        }
        #[cfg(target_os = "windows")]
        if field_type == "identifier" || field_type == "password" {
            return self.type_credential_on_windows(page, value, field_type == "password").await;
        }

        // Keep the selected node, not whichever input happens to gain focus next.
        let target = page.find_element("input:focus").await.map_err(|_| {
            crate::error::VaultError::SmartLoginError("Credential field lost focus".into())
        })?;
        let expected_password = field_type == "password";
        let guard_js = format!(r#"function() {{
            return this === document.activeElement && this.isConnected &&
                !this.disabled && !this.readOnly && !this.matches(':disabled') &&
                (this.type === 'password') === {expected_password} &&
                this.getClientRects().length > 0 &&
                (!this.checkVisibility || this.checkVisibility({{checkOpacity:true,checkVisibilityCSS:true}}));
        }}"#);
        let clear_js = format!(r#"function() {{
            if (!({guard_js}).call(this)) return false;
            this.value = '';
            this.dispatchEvent(new Event('input', {{bubbles:true,composed:true}}));
            return ({guard_js}).call(this);
        }}"#);
        let cleared = target.call_js_fn(clear_js, false).await.map_err(|_| {
            crate::error::VaultError::SmartLoginError("Could not clear credential field".into())
        })?;
        if cleared.result.value.and_then(|value| value.as_bool()) != Some(true) {
            return Err(crate::error::VaultError::SmartLoginError("Credential field changed before typing".into()));
        }
        for ch in value.chars() {
            if self.is_cancelled() {
                return Err(crate::error::VaultError::SmartLoginError("Login cancelled".into()));
            }
            let valid = target.call_js_fn(guard_js.clone(), false).await.map_err(|_| {
                crate::error::VaultError::SmartLoginError("Cannot verify credential field".into())
            })?;
            if valid.result.value.and_then(|value| value.as_bool()) != Some(true) {
                return Err(crate::error::VaultError::SmartLoginError("Credential field lost focus".into()));
            }
            let text = Zeroizing::new(ch.to_string());
            target.type_str(&*text).await.map_err(|_| {
                // CDP errors can contain the character; never include them in a login log.
                crate::error::VaultError::SmartLoginError("Could not type into credential field".into())
            })?;
            let (min_delay, max_delay) = self.config.keystroke_delay_range_ms;
            let delay = if min_delay < max_delay {
                use rand::Rng;
                rand::rng().random_range(min_delay..max_delay)
            } else { min_delay };
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        }
        let finish_js = format!(r#"function() {{
            if (!({guard_js}).call(this)) return false;
            for (const type of ['input','change','blur']) {{
                if (!({guard_js}).call(this)) return false;
                this.dispatchEvent(new Event(type, {{bubbles:true,composed:true}}));
            }}
            return true;
        }}"#);
        let finished = target.call_js_fn(finish_js, false).await.map_err(|_| {
            crate::error::VaultError::SmartLoginError("Cannot finalize credential input".into())
        })?;
        if finished.result.value.and_then(|value| value.as_bool()) != Some(true) {
            return Err(crate::error::VaultError::SmartLoginError("Credential field changed after typing".into()));
        }
        self.logger.emit(
            SmartLoginEvent::new(LoginState::FillingIdentifier, format!("{field_type} filled [REDACTED]"))
                .with_detail(SmartLoginEventDetail::Interaction {
                    action: "fill".into(),
                    target_description: field_type.into(),
                }),
        );

        Ok(())
    }

    #[cfg(target_os = "windows")]
    async fn type_credential_on_windows(
        &self,
        page: &Page,
        value: &Zeroizing<String>,
        is_password: bool,
    ) -> crate::Result<()> {
        use crate::services::autotype::{foreground_browser_token, type_browser_field};
        let window = foreground_browser_token()?;
        // CDP remains responsible for selecting the already domain-checked page.
        // Require visible page focus before sending any global OS input.
        let check_js = format!(
            r#"(() => {{
                const el = document.activeElement;
                if (!document.hasFocus() || !(el instanceof HTMLInputElement) ||
                    el.disabled || el.readOnly ||
                    (el.type === 'password') !== {} ||
                    el.getClientRects().length === 0) return null;
                return el.id || '';
            }})()"#,
            is_password
        );
        let result = page.evaluate(check_js).await.map_err(|_| {
            crate::error::VaultError::SmartLoginError("Cannot verify browser input focus".into())
        })?;
        let field_id = result
            .into_value::<Option<String>>()
            .ok()
            .flatten()
            .ok_or_else(|| {
                crate::error::VaultError::SmartLoginError(if is_password {
                    "Focus the browser password field".into()
                } else {
                    "Focus the browser username field".into()
                })
            })?;
        if self.cancel_flag.load(Ordering::SeqCst) {
            return Err(crate::error::VaultError::SmartLoginError("Login cancelled".into()));
        }
        let text = value.clone();
        let expected_url = page.url().await.map_err(|_| crate::error::VaultError::SmartLoginError("Cannot verify page address".into()))?
            .ok_or_else(|| crate::error::VaultError::SmartLoginError("Missing page address".into()))?;
        let char_delay_ms = self.config.keystroke_delay_range_ms.0;
        let cancelled = self.cancel_flag.clone();
        tokio::task::spawn_blocking(move || {
            if cancelled.load(Ordering::SeqCst) {
                return Err(crate::error::VaultError::SmartLoginError("Login cancelled".into()));
            }
            type_browser_field(&text, window, &field_id, is_password, char_delay_ms, &expected_url)
        })
        .await
        .map_err(|_| crate::error::VaultError::SmartLoginError("Browser typing interrupted".into()))??;
        if self.cancel_flag.load(Ordering::SeqCst) {
            return Err(crate::error::VaultError::SmartLoginError("Login cancelled".into()));
        }
        let (state, label) = if is_password {
            (LoginState::FillingPassword, "Password typed into protected field [REDACTED]")
        } else {
            (LoginState::FillingIdentifier, "Identifier typed and verified [REDACTED]")
        };
        self.logger.log(state, label);
        Ok(())
    }

    /// Click the most likely submit/next/continue button on the page.
    /// Scopes to the form containing the focused input first.
    async fn direct_submit(&self, page: &Page) {
        #[cfg(target_os = "windows")]
        {
            use crate::services::autotype::{foreground_browser_token, send_enter_guarded, verify_browser_submit};
            if let Ok(token) = foreground_browser_token() {
                if self.is_cancelled() || !page.evaluate("document.hasFocus() && document.activeElement instanceof HTMLInputElement").await
                    .ok().and_then(|v| v.into_value::<bool>().ok()).unwrap_or(false) {
                    return;
                }
                let Some(url) = page.url().await.ok().flatten() else { return; };
                if !tokio::task::spawn_blocking(move || verify_browser_submit(token, &url)).await.unwrap_or(false) { return; }
                let hwnd = windows::Win32::Foundation::HWND(token as *mut _);
                // First try submitting via native physical Enter keystroke directly to the focused input.
                // OS keyboard events preserve normal form submission behavior;
                // sites can still require a challenge or reject the session.
                if send_enter_guarded(hwnd).is_ok() {
                    self.logger.log(LoginState::SubmittingLogin, "Submitted via native Enter keystroke");
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    return;
                }
            }
        }
        let submit_js = r#"
        (() => {
            const loginPatterns = [
                'sign in', 'signin', 'log in', 'login', 'logga in', 'anmelden',
                'submit', 'entrar', 'accedi', 'войти',
            ];
            const continuePatterns = [
                'next', 'continue', 'proceed', 'verify', 'authenticate',
                'nästa', 'fortsätt', 'bekräfta', 'autentisera', 'weiter', 'continuar', 'continuer', 'suivant',
                'avanti', 'próximo', 'volgende', 'dalej', 'далее',
            ];
            const allPatterns = [...loginPatterns, ...continuePatterns];

            // Excluded words — buttons that are NOT submit actions
            const excludePatterns = [
                'enterprise', 'pricing', 'features', 'about', 'contact',
                'help', 'support', 'privacy', 'terms', 'blog', 'docs',
                'sign up', 'signup', 'register', 'create account',
                'forgot', 'reset', 'cancel',
            ];

            function isExcluded(text) {
                return excludePatterns.some(p => text.includes(p));
            }

            function matchButton(btn) {
                const text = (btn.textContent || btn.value || '').trim().toLowerCase();
                if (isExcluded(text)) return null;
                for (const pattern of allPatterns) {
                    if (text === pattern || (text.includes(pattern) && text.length < pattern.length + 15)) {
                        return text;
                    }
                }
                const label = (btn.getAttribute('aria-label') || '').toLowerCase();
                if (label && !isExcluded(label)) {
                    for (const pattern of allPatterns) {
                        if (label.includes(pattern)) return label;
                    }
                }
                return null;
            }

            // Strategy 1: Find submit button INSIDE the form of the active element or input field
            let focused = document.activeElement;
            if (!focused || focused === document.body || focused.tagName !== 'INPUT') {
                focused = document.querySelector('input[autocomplete="one-time-code"], input[name*="otp" i], input[name*="totp" i], input[name*="app_totp" i], input[type="password"]:not([hidden]), input[type="email"]:not([hidden]), input[type="text"]:not([hidden])');
            }
            const form = focused ? focused.closest('form') : null;
            if (form) {
                // Try form's submit input first
                const submitInput = form.querySelector('input[type="submit"]');
                if (submitInput) {
                    submitInput.click();
                    return 'clicked: ' + (submitInput.value || 'submit');
                }
                // Try buttons inside the form
                const formButtons = form.querySelectorAll('button, [role="button"]');
                for (const btn of formButtons) {
                    const match = matchButton(btn);
                    if (match) { btn.click(); return 'clicked: ' + match; }
                }
                // Submit the form directly
                const submitBtn = form.querySelector('button[type="submit"]');
                if (submitBtn) { submitBtn.click(); return 'clicked: form submit btn'; }
                if (typeof form.requestSubmit === 'function') {
                    form.requestSubmit();
                } else {
                    form.submit();
                }
                return 'form_submitted';
            }

            // Strategy 2: Page-wide button search (no form context)
            const buttons = document.querySelectorAll(
                'button, input[type="submit"], input[type="button"], [role="button"]'
            );
            for (const btn of buttons) {
                const match = matchButton(btn);
                if (match) { btn.click(); return 'clicked: ' + match; }
            }

            // Strategy 2b: Fallback to any visible submit button on the page
            const submitBtnFallback = document.querySelector('button[type="submit"]:not([hidden]), input[type="submit"]:not([hidden])');
            if (submitBtnFallback) {
                submitBtnFallback.click();
                return 'clicked: fallback submit button';
            }

            // Strategy 3: Press Enter on targeted input element
            if (focused) {
                focused.dispatchEvent(new KeyboardEvent('keydown', {key: 'Enter', code: 'Enter', keyCode: 13, bubbles: true}));
                focused.dispatchEvent(new KeyboardEvent('keypress', {key: 'Enter', code: 'Enter', keyCode: 13, bubbles: true}));
                focused.dispatchEvent(new KeyboardEvent('keyup', {key: 'Enter', code: 'Enter', keyCode: 13, bubbles: true}));
                return 'enter_pressed';
            }

            return 'no_button';
        })()
        "#;

        let result = page.evaluate(submit_js).await;
        if let Ok(val) = result
            && let Ok(status) = val.into_value::<String>() {
                self.logger.log(LoginState::SubmittingLogin, format!("Submit: {status}"));
            }
    }

    /// Try to click an account chooser button (e.g., "Use another account").
    /// Returns true if a button was clicked.
    async fn try_account_chooser(&self, page: &Page) -> bool {
        let js = r#"
        (() => {
            const patterns = [
                'use another', 'another account', 'add account', 'add an account',
                'lägg till', 'annat konto', 'anderes konto', 'otro cuenta',
                'autre compte', 'use a different',
            ];
            const elements = document.querySelectorAll(
                'button, [role="button"], [role="link"], a, li[data-identifier], div[data-identifier]'
            );
            for (const el of elements) {
                const text = (el.textContent || '').trim().toLowerCase();
                for (const pattern of patterns) {
                    if (text.includes(pattern)) {
                        el.click();
                        return 'clicked: ' + text;
                    }
                }
            }
            return 'not_found';
        })()
        "#;

        if let Ok(result) = page.evaluate(js).await
            && let Ok(val) = result.into_value::<String>() {
                return val.starts_with("clicked");
            }
        false
    }


    fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::Relaxed)
    }
}
