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

/// Top-level facade for executing a Smart Login attempt.
pub struct SmartLoginEngine {
    config: SmartLoginConfig,
    logger: SmartLoginLogger,
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
            logger,
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
        self.logger.log(LoginState::Idle, format!("Starting Smart Login for {url}"));

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

        // Phase 2: Analyze page and find/navigate to login form
        let page = match self.find_login_form(page, url).await {
            Ok(p) => p,
            Err(result) => return result,
        };

        if self.is_cancelled() {
            return LoginResult::Cancelled;
        }

        // Phase 3: Fill credentials and submit
        let result = self
            .fill_and_submit(&page, url, identifier, password)
            .await;

        // Phase 4: Handle TOTP if MFA is required OR if totp_secret exists and page has an OTP field
        let result = match (&result, &totp_secret) {
            (LoginResult::RequiresMfa { .. }, Some(secret)) => {
                self.handle_totp(&page, url, secret).await
            }
            (res, Some(secret)) => {
                // Secondary check: check if page currently contains an OTP field
                if self.has_otp_field(&page).await {
                    self.handle_totp(&page, url, secret).await
                } else {
                    res.clone()
                }
            }
            _ => result,
        };

        // Disconnect CDP so the browser resumes normal operation
        session.disconnect();

        result
    }

    /// Check if the page currently contains an OTP / 2FA input field.
    async fn has_otp_field(&self, page: &Page) -> bool {
        let js = r#"
        (() => {
            const selectors = [
                'input[autocomplete="one-time-code"]',
                'input[name*="otp" i]', 'input[name*="totp" i]', 'input[name*="mfa" i]',
                'input[name*="2fa" i]', 'input[name*="code" i]', 'input[name*="pin" i]',
                'input[name*="otc" i]', 'input[name*="two_factor" i]', 'input[name*="app_totp" i]',
                'input[id*="otp" i]', 'input[id*="totp" i]', 'input[id*="mfa" i]',
                'input[id*="2fa" i]', 'input[id*="code" i]', 'input[id*="pin" i]',
                'input[id*="otc" i]', 'input[id*="app_totp" i]', 'input[inputmode="numeric"]',
            ];
            for (const sel of selectors) {
                const el = document.querySelector(sel);
                if (el && el.offsetParent !== null) return true;
            }
            const inputs = document.querySelectorAll('input[type="text"], input[type="tel"], input[type="number"]');
            for (const el of inputs) {
                if (el.offsetParent !== null && (el.maxLength <= 8 || el.getAttribute('inputmode') === 'numeric')) {
                    return true;
                }
            }
            return false;
        })()
        "#;
        page.evaluate(js).await.ok().and_then(|v| v.into_value::<bool>().ok()).unwrap_or(false)
    }

    /// Generate and fill a TOTP code when 2FA is required.
    async fn handle_totp(
        &self,
        page: &Page,
        entry_url: &str,
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

        match verifier::verify_login_result(page, &snapshot_url, entry_url, &self.logger).await {
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
                if (el.offsetParent !== null && !el.value && (el.maxLength <= 8 || el.getAttribute('inputmode') === 'numeric')) {
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
        if status == "not_found" {
            return Err(crate::error::VaultError::SmartLoginError(
                "No OTP input field found on page".into(),
            ));
        }

        // Type character by character into document.activeElement using JS 
        // to avoid Chromium CDP "Lost Focus" pseudo-class issues when the site auto-advances.
        for ch in code.chars() {
            let type_js = format!(
                r#"
                (() => {{
                    const el = document.activeElement;
                    if (el && el.tagName === 'INPUT') {{
                        // Append character to value if it's a single box, or set it if it's a multi-box
                        if (el.maxLength === 1) {{
                            el.value = '{}';
                        }} else {{
                            el.value += '{}';
                        }}
                        el.dispatchEvent(new Event('input', {{bubbles: true, composed: true}}));
                        
                        // Fire a simulated keydown/keyup so React/Vue auto-advance scripts trigger!
                        el.dispatchEvent(new KeyboardEvent('keydown', {{key: '{}', bubbles: true}}));
                        el.dispatchEvent(new KeyboardEvent('keyup', {{key: '{}', bubbles: true}}));
                    }}
                }})()
                "#,
                ch, ch, ch, ch
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
    async fn find_login_form(&self, page: Page, entry_url: &str) -> Result<Page, LoginResult> {
        let current_page = page;
        let mut nav_attempts = 0;

        loop {
            if self.is_cancelled() {
                return Err(LoginResult::Cancelled);
            }

            // Wait for page to have interactive content before analyzing
            analyzer::wait_for_page_ready(&current_page, 8000).await;

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

            // Detect already-logged-in state (dashboard/inbox with no login form)
            if discovery::is_likely_logged_in(&snapshot) && nav_attempts == 0 {
                self.logger.log(
                    LoginState::AnalyzingPage,
                    "Already logged in — looking for account switcher or login page",
                );
            }

            // Try login link candidates
            let candidates = discovery::find_login_links(&snapshot, &self.logger);

            if !candidates.is_empty() {
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

                            // Re-analyze after probe navigation
                            if let Ok(snap) = analyzer::analyze_page(&current_page, &self.logger).await {
                                if discovery::has_login_form(&snap) {
                                    self.logger.log(
                                        LoginState::FormDetected,
                                        format!("Login form found at {probe_url}"),
                                    );
                                    return Ok(current_page);
                                }
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
        identifier: Zeroizing<String>,
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
            let has_password_input = snapshot.inputs.iter().any(|i| i.input_type == "password");
            let has_identifier_input = snapshot.inputs.iter().any(|i| {
                matches!(i.input_type.as_str(), "email" | "tel")
                    || (i.input_type == "text" && !self.is_search_field(i))
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

                return match verifier::verify_login_result(page, &snapshot.url, entry_url, &self.logger).await {
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

                return match verifier::verify_login_result(page, &snapshot.url, entry_url, &self.logger).await {
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
        // Find the target field via JS
        let find_js = if field_type == "password" {
            // Password: just find input[type="password"]
            r#"
            (() => {
                const el = document.querySelector('input[type="password"]:not([hidden])');
                if (el) { el.focus(); el.click(); return 'found'; }
                return 'not_found';
            })()
            "#.to_string()
        } else {
            // Identifier: prioritize email > tel > text (excluding search)
            r#"
            (() => {
                const selectors = [
                    'input[type="email"]:not([hidden])',
                    'input[type="tel"]:not([hidden])',
                    'input[autocomplete="username"]:not([hidden])',
                    'input[name*="identifier"]:not([hidden])',
                    'input[name*="email"]:not([hidden])',
                    'input[name*="user"]:not([hidden])',
                    'input[name*="login"]:not([hidden])',
                    'input[id*="identifier"]:not([hidden])',
                    'input[id*="email"]:not([hidden])',
                    'input[id*="user"]:not([hidden])',
                    'input[id*="login"]:not([hidden])',
                ];
                for (const sel of selectors) {
                    const el = document.querySelector(sel);
                    if (el && el.offsetParent !== null) {
                        el.focus();
                        el.click();
                        return 'found';
                    }
                }
                // Fallback: first visible text input that isn't search
                const texts = document.querySelectorAll('input[type="text"]:not([hidden])');
                for (const el of texts) {
                    const name = (el.name + ' ' + el.id + ' ' + el.placeholder).toLowerCase();
                    if (!name.includes('search') && !name.includes('query') && el.offsetParent !== null) {
                        el.focus();
                        el.click();
                        return 'found';
                    }
                }
                return 'not_found';
            })()
            "#.to_string()
        };

        let result = page.evaluate(find_js).await.map_err(|e| {
            crate::error::VaultError::SmartLoginError(format!("Field find failed: {e}"))
        })?;
        let status: String = result.into_value().unwrap_or_else(|_| "error".into());
        if status == "not_found" {
            return Err(crate::error::VaultError::SmartLoginError(
                format!("No {field_type} field found on page"),
            ));
        }

        // Clear existing value
        let _ = page.evaluate(r#"
            (() => {
                const el = document.activeElement;
                if (el && el.tagName === 'INPUT') {
                    el.value = '';
                    el.dispatchEvent(new Event('input', {bubbles: true, composed: true}));
                }
            })()
        "#).await;

        // Type character by character via CDP keyboard events
        for ch in value.chars() {
            let mut text = String::new();
            text.push(ch);

            // Type via focused element using CDP keyboard events
            let focused = page.find_element("input:focus").await.map_err(|e| {
                crate::error::VaultError::SmartLoginError(format!("Lost focus: {e}"))
            })?;
            focused.type_str(&text).await.map_err(|e| {
                crate::error::VaultError::SmartLoginError(format!("Typing failed: {e}"))
            })?;

            // Random keystroke delay
            let (min_delay, max_delay) = self.config.keystroke_delay_range_ms;
            let delay = if min_delay < max_delay {
                use rand::Rng;
                let mut rng = rand::rng();
                rng.random_range(min_delay..max_delay)
            } else {
                min_delay
            };
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        }

        // Dispatch Bitwarden-style events to notify frameworks
        let _ = page.evaluate(r#"
            (() => {
                const el = document.activeElement;
                if (el && el.tagName === 'INPUT') {
                    el.dispatchEvent(new Event('input', {bubbles: true, composed: true}));
                    el.dispatchEvent(new Event('change', {bubbles: true, composed: true}));
                    el.dispatchEvent(new Event('blur', {bubbles: true, composed: true}));
                }
            })()
        "#).await;

        self.logger.emit(
            SmartLoginEvent::new(LoginState::FillingIdentifier, format!("{field_type} filled [REDACTED]"))
                .with_detail(SmartLoginEventDetail::Interaction {
                    action: "fill".into(),
                    target_description: field_type.into(),
                }),
        );

        Ok(())
    }

    /// Click the most likely submit/next/continue button on the page.
    /// Scopes to the form containing the focused input first.
    async fn direct_submit(&self, page: &Page) {
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
        if let Ok(val) = result {
            if let Ok(status) = val.into_value::<String>() {
                self.logger.log(LoginState::SubmittingLogin, format!("Submit: {status}"));
            }
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

        if let Ok(result) = page.evaluate(js).await {
            if let Ok(val) = result.into_value::<String>() {
                return val.starts_with("clicked");
            }
        }
        false
    }


    fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::Relaxed)
    }
}
