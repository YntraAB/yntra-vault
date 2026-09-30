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
        if crate::services::network::ensure_allowed().is_err() {
            return LoginResult::BrowserError("Network access is disabled".into());
        }
        if discovery::credential_url(url).is_none() {
            return LoginResult::BrowserError("Smart Login requires a valid HTTPS website without URL credentials".into());
        }
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
        let fill_result = self.direct_fill_otp(page, entry_url, &totp_code).await;
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
        self.direct_submit(page, entry_url).await;

        let snapshot_url = page.evaluate("window.location.href").await
            .ok()
            .and_then(|v| v.into_value::<String>().ok())
            .unwrap_or_default();

        match verifier::verify_login_result(page, &snapshot_url, entry_url, identifier, true, &self.cancel_flag, &self.logger).await {
            Ok(result) => result,
            Err(e) => LoginResult::BrowserError(format!("Post-TOTP verification failed: {e}")),
        }
    }

    async fn verify_fill_origin(&self, page: &Page, entry_url: &str) -> crate::Result<String> {
        crate::services::network::ensure_allowed()?;
        if self.is_cancelled() { return Err(crate::error::VaultError::SmartLoginError("Login cancelled".into())); }
        let current = page.evaluate("window.location.href").await
            .map_err(|_| crate::error::VaultError::SmartLoginError("Cannot verify website before typing".into()))?
            .into_value::<String>().map_err(|_| crate::error::VaultError::SmartLoginError("Invalid website address".into()))?;
        if !discovery::is_allowed_auth_domain(entry_url, &current) {
            return Err(crate::error::VaultError::SmartLoginError("Website changed before typing; sign-in stopped".into()));
        }
        Ok(current)
    }
    /// Fill a TOTP/OTP input field visually (simulating human typing).
    async fn direct_fill_otp(
        &self,
        page: &Page,
        entry_url: &str,
        code: &Zeroizing<String>,
    ) -> crate::Result<()> {
        let expected_url = self.verify_fill_origin(page, entry_url).await?;
        let expected_origin = serde_json::to_string(&discovery::credential_url(&expected_url).unwrap().origin().ascii_serialization()).unwrap();
        let focus_js = r#"
        (() => {
            const walk = (root, seen = new Set()) => {
                if (!root || seen.has(root)) return [];
                seen.add(root);
                const nodes = [];
                for (const el of root.querySelectorAll ? root.querySelectorAll('*') : []) {
                    nodes.push(el);
                    if (el.shadowRoot) nodes.push(...walk(el.shadowRoot, seen));
                    if (el.tagName === 'IFRAME') {
                        try { if (el.contentDocument) nodes.push(...walk(el.contentDocument, seen)); } catch (_) {}
                    }
                }
                return nodes;
            };
            const all = walk(document);
            // Strategy 1: Multi-box input (e.g. 6 separate inputs)
            const singleCharInputs = all.filter(el => el.tagName === 'INPUT').filter(el =>
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
                const el = all.find(candidate => candidate.matches(sel));
                if (el && el.offsetParent !== null) {
                    el.focus();
                    el.click();
                    return 'found';
                }
            }

            // Strategy 3: Fallback any short text input
            const inputs = all.filter(candidate => candidate.matches('input[type="text"], input[type="tel"], input[type="number"]'));
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
            self.verify_fill_origin(page, entry_url).await?;
            let ch_json = serde_json::to_string(&ch.to_string()).unwrap_or_else(|_| "\"\"".into());
            let type_js = format!(
                r#"
                (() => {{
                    const deepActive = (root) => {{
                        let el = root && root.activeElement;
                        while (el) {{
                            if (el.shadowRoot?.activeElement) {{ el = el.shadowRoot.activeElement; continue; }}
                            if (el.tagName === 'IFRAME') {{
                                try {{ const inner = el.contentDocument?.activeElement; if (inner && inner !== el) {{ el = inner; continue; }} }} catch (_) {{}}
                            }}
                            break;
                        }}
                        return el;
                    }};
                    const el = deepActive(document);
                    if (el && (el.ownerDocument.defaultView || window).location.origin === {expected_origin} &&
                        el.tagName === 'INPUT' && !el.disabled && !el.readOnly && el.getClientRects().length) {{
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
                const deepActive = (root) => {
                    let el = root && root.activeElement;
                    while (el) {
                        if (el.shadowRoot?.activeElement) { el = el.shadowRoot.activeElement; continue; }
                        if (el.tagName === 'IFRAME') {
                            try { const inner = el.contentDocument?.activeElement; if (inner && inner !== el) { el = inner; continue; } } catch (_) {}
                        }
                        break;
                    }
                    return el;
                };
                const el = deepActive(document);
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
                let _ = current_page.bring_to_front().await;
                self.logger
                    .log(LoginState::FormDetected, "Login form detected on page");
                return Ok(current_page);
            }

            // A conventional login URL is already the user's intended flow.
            // Keep it in place for dynamic or unfamiliar controls instead of
            // probing unrelated /signin, /auth or account paths.
            if discovery::is_login_url(&snapshot.url) {
                let _ = current_page.bring_to_front().await;
                self.logger.log(
                    LoginState::FormDetected,
                    "Login URL reached; keeping page for dynamic controls",
                );
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
                    if !discovery::login_target_allowed(entry_url, probe_url) {
                        self.logger.log(
                            LoginState::SearchingForLogin,
                            "Skipping a login probe outside the verified origin boundary",
                        );
                        continue;
                    }
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
        let mut method_selector_clicks = 0;

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
            let _ = page.bring_to_front().await;

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
                i.is_visible && !i.is_readonly && (matches!(i.input_type.as_str(), "email" | "tel" | "url" | "number")
                    || (i.input_type == "text" && !self.is_search_field(i)))
            });

            // Some sign-in pages expose only one identifier method at a time
            // (for example phone first, with a separate "use email instead"
            // control). Switch only when the visible field clearly does not
            // match the supplied credential and the control has an explicit
            // identifier label.
            let identifier_kind_matches = snapshot.inputs.iter().any(|i| {
                i.is_visible && !i.is_readonly && self.identifier_field_matches(i, identifier)
            });
            if method_selector_clicks < 2 && (!has_identifier_input || !identifier_kind_matches)
                && self.try_login_method_selector(page, entry_url, identifier).await
            {
                method_selector_clicks += 1;
                self.logger.log(LoginState::SelectingLoginMethod, "Selected the matching login method");
                analyzer::wait_for_page_ready(page, 5000).await;
                continue;
            }

            // We are on a recognized login chooser, but its method control is
            // unfamiliar or unavailable. Stay on this trusted page and ask for
            // a manual choice instead of probing unrelated URLs.
            if discovery::has_login_method_chooser(&snapshot) && !has_identifier_input && !has_password_input {
                self.logger.log(
                    LoginState::RequiresManualAction(ManualActionType::UnknownChallenge),
                    "Login method chooser needs a manual selection",
                );
                return LoginResult::RequiresManualAction;
            }

            // Single-step form: both identifier AND password on same page
            if has_identifier_input && has_password_input {
                self.logger.log(LoginState::FillingIdentifier, "Filling identifier...");
                let fill_result = self.direct_fill_field(page, entry_url, "identifier", &identifier).await;
                if let Err(e) = fill_result {
                    return LoginResult::BrowserError(format!("Identifier fill failed: {e}"));
                }

                self.logger.log(LoginState::FillingPassword, "Filling password...");
                let fill_result = self.direct_fill_field(page, entry_url, "password", &password).await;
                if let Err(e) = fill_result {
                    return LoginResult::BrowserError(format!("Password fill failed: {e}"));
                }

                self.logger.log(LoginState::SubmittingLogin, "Submitting...");
                self.direct_submit(page, entry_url).await;

                return match verifier::verify_login_result(page, &snapshot.url, entry_url, identifier, false, &self.cancel_flag, &self.logger).await {
                    Ok(result) => result,
                    Err(e) => LoginResult::BrowserError(format!("Verification failed: {e}")),
                };
            }

            // Multi-step second page: password only (identifier already submitted)
            if has_password_input {
                self.logger.log(LoginState::FillingPassword, "Filling password...");
                let fill_result = self.direct_fill_field(page, entry_url, "password", &password).await;
                if let Err(e) = fill_result {
                    return LoginResult::BrowserError(format!("Password fill failed: {e}"));
                }

                self.logger.log(LoginState::SubmittingLogin, "Submitting...");
                self.direct_submit(page, entry_url).await;

                return match verifier::verify_login_result(page, &snapshot.url, entry_url, identifier, false, &self.cancel_flag, &self.logger).await {
                    Ok(result) => result,
                    Err(e) => LoginResult::BrowserError(format!("Verification failed: {e}")),
                };
            }

            // Multi-step first page: identifier only → click Next
            if has_identifier_input {
                self.logger.log(LoginState::FillingIdentifier, "Filling identifier...");
                let fill_result = self.direct_fill_field(page, entry_url, "identifier", &identifier).await;
                if let Err(e) = fill_result {
                    return LoginResult::BrowserError(format!("Identifier fill failed: {e}"));
                }

                self.logger.log(LoginState::SubmittingIdentifier, "Clicking Next...");
                self.direct_submit(page, entry_url).await;

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

            if discovery::is_login_url(&snapshot.url) {
                self.logger.log(
                    LoginState::RequiresManualAction(ManualActionType::UnknownChallenge),
                    "Login page needs a manual control selection",
                );
                return LoginResult::RequiresManualAction;
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

    fn identifier_field_matches(&self, input: &InputInfo, identifier: &str) -> bool {
        let descriptive = format!("{} {} {} {} {}", input.input_type, input.name, input.id,
            input.placeholder, input.aria_label).to_lowercase();
        let autocomplete = input.autocomplete.to_lowercase();
        let phone_hint = ["phone", "mobile", "telephone", "tel", "sms"].iter()
            .any(|pattern| descriptive.contains(pattern));
        let email_hint = descriptive.contains("email") || descriptive.contains("e-mail")
            || descriptive.contains("mail");
        let username_hint = ["username", "user name", "user", "login", "identifier"].iter()
            .any(|pattern| descriptive.contains(pattern));
        let username_autocomplete = autocomplete.contains("username");
        let code_hint = ["code", "otp", "verification", "one-time", "pin"].iter()
            .any(|pattern| descriptive.contains(pattern));
        if identifier.contains('@') {
            input.input_type == "email" || (!phone_hint && (email_hint || username_hint || username_autocomplete))
        } else if identifier.chars().all(|c| c.is_ascii_digit() || matches!(c, '+' | ' ' | '-' | '(' | ')'))
            && identifier.chars().any(|c| c.is_ascii_digit()) {
            input.input_type == "tel" || phone_hint
        } else {
            if code_hint || phone_hint || email_hint {
                return username_hint;
            }
            input.input_type == "text" || input.input_type == "url" || username_hint || username_autocomplete
        }
    }

    /// Select an explicitly labelled email/username/phone method on pages that
    /// render only one identifier field at a time. The page remains origin
    /// checked and the selector is limited to visible, interactive controls.
    async fn try_login_method_selector(
        &self,
        page: &Page,
        entry_url: &str,
        identifier: &str,
    ) -> bool {
        if self.verify_fill_origin(page, entry_url).await.is_err() { return false; }
        let method = if identifier.contains('@') {
            "email"
        } else if identifier.chars().all(|c| c.is_ascii_digit() || matches!(c, '+' | ' ' | '-' | '(' | ')'))
            && identifier.chars().any(|c| c.is_ascii_digit()) {
            "phone"
        } else {
            "username"
        };
        let method_json = serde_json::to_string(method).unwrap_or_else(|_| "\"username\"".into());
        let js = format!(r#"
        (() => {{
            const wanted = {method_json};
            const labels = {{
                email: ['email', 'e-mail', 'mail', 'email address', 'use email', 'email instead', 'correo', 'correo electrónico', 'courriel', 'e-post', 'e-mailadresse', 'adresse e-mail', '邮箱', 'メール'],
                username: ['username', 'user name', 'user id', 'login name', 'use username', 'username instead', 'usuario', 'utilisateur', 'användarnamn', 'benutzername', 'nome de usuário', 'имя пользователя', '用户名'],
                phone: ['phone', 'phone number', 'mobile', 'telephone', 'use phone', 'mobile number', 'teléfono', 'número de teléfono', 'téléphone', 'telefon', 'telefonnummer', 'mobilnummer', 'телефон', '手机号码']
            }}[wanted];
            const excluded = ['sign up', 'signup', 'register', 'create account', 'forgot', 'help', 'support'];
            const walk = (root, seen = new Set()) => {{
                if (!root || seen.has(root)) return [];
                seen.add(root);
                const nodes = [];
                for (const el of root.querySelectorAll ? root.querySelectorAll('*') : []) {{
                    nodes.push(el);
                    if (el.shadowRoot) nodes.push(...walk(el.shadowRoot, seen));
                    if (el.tagName === 'IFRAME') {{ try {{ if (el.contentDocument) nodes.push(...walk(el.contentDocument, seen)); }} catch (_) {{}} }}
                }}
                return nodes;
            }};
            const visible = el => {{
                if (!el || el.disabled || el.matches(':disabled,[aria-disabled="true"]')) return false;
                const style = (el.ownerDocument.defaultView || window).getComputedStyle(el), rect = el.getBoundingClientRect();
                return style.display !== 'none' && style.visibility !== 'hidden' && style.opacity !== '0' &&
                    rect.width > 0 && rect.height > 0 && (!el.checkVisibility || el.checkVisibility({{checkOpacity:true,checkVisibilityCSS:true}}));
            }};
            const controls = walk(document).filter(el => el.matches('button, input[type="button"], input[type="submit"], a, [role="button"], [role="tab"], [role="link"], [tabindex="0"], summary'));
            for (const el of controls) {{
                if (!visible(el)) continue;
                const text = (el.getAttribute('aria-label') || el.getAttribute('title') || el.textContent || '').trim().toLowerCase();
                if (!text || excluded.some(word => text.includes(word))) continue;
                if (labels.some(label => text === label || text.includes(label))) {{
                    el.click();
                    return true;
                }}
            }}
            return false;
        }})()
        "#);
        page.evaluate(js).await.ok().and_then(|v| v.into_value::<bool>().ok()).unwrap_or(false)
    }

    /// Fill a field using direct JS injection with Bitwarden-style event dispatch.
    /// `field_type` is "identifier" or "password".
    async fn direct_fill_field(
        &self,
        page: &Page,
        entry_url: &str,
        field_type: &str,
        value: &Zeroizing<String>,
    ) -> crate::Result<()> {
        let expected_url = self.verify_fill_origin(page, entry_url).await?;
        let expected_origin = serde_json::to_string(&discovery::credential_url(&expected_url).unwrap().origin().ascii_serialization()).unwrap();
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
            return self.type_credential_on_windows(page, entry_url, value, field_type == "password").await;
        }

        // The semantic selector can focus fields inside open shadow roots and
        // same-origin frames. Keep that deep-active element instead of falling
        // back to the top-document input list.
        let marker = format!("yntra-vault-{}", uuid::Uuid::new_v4());
        let marker_json = serde_json::to_string(&marker).unwrap();
        let expected_password = field_type == "password";
        let bind_js = format!(r#"
            (() => {{
                const expectedOrigin = {expected_origin};
                const expectedPassword = {expected_password};
                const marker = {marker_json};
                const deepActive = (root) => {{
                    let el = root && root.activeElement;
                    while (el) {{
                        if (el.shadowRoot && el.shadowRoot.activeElement) {{ el = el.shadowRoot.activeElement; continue; }}
                        if (el.tagName === 'IFRAME') {{
                            try {{ const inner = el.contentDocument && el.contentDocument.activeElement; if (inner && inner !== el) {{ el = inner; continue; }} }} catch (_) {{}}
                        }}
                        break;
                    }}
                    return el;
                }};
                const visible = (el) => {{
                    if (!el || !el.isConnected || el.closest('[inert]')) return false;
                    const style = (el.ownerDocument.defaultView || window).getComputedStyle(el);
                    return style.display !== 'none' && style.visibility !== 'hidden' && style.opacity !== '0' &&
                        el.getClientRects().length > 0 && (!el.checkVisibility || el.checkVisibility({{checkOpacity:true,checkVisibilityCSS:true}}));
                }};
                const editable = (el) => el && !el.disabled && !el.readOnly && !el.matches(':disabled') &&
                    ((el.tagName === 'INPUT' && (expectedPassword ? el.type === 'password' : ['email','tel','text','url','number'].includes(el.type))) ||
                     (!expectedPassword && (el.tagName === 'TEXTAREA' || el.isContentEditable || el.getAttribute('role') === 'textbox')));
                const target = deepActive(document);
                if (!editable(target) || !visible(target) || (target.ownerDocument.defaultView || window).location.origin !== expectedOrigin ||
                    (target.tagName === 'INPUT' && (target.type === 'password') !== expectedPassword)) return false;
                if (target.__yntraVaultFillMarker && target.__yntraVaultFillMarker !== marker) return false;
                target.__yntraVaultFillMarker = marker;
                return true;
            }})()
        "#);
        let bound = page.evaluate(bind_js).await.map_err(|_| {
            crate::error::VaultError::SmartLoginError("Could not bind the verified credential field".into())
        })?;
        if bound.into_value::<bool>().ok() != Some(true) {
            return Err(crate::error::VaultError::SmartLoginError("Credential field changed before typing".into()));
        }

        let clear_js = format!(r#"
            (() => {{
                const expectedOrigin = {expected_origin};
                const expectedPassword = {expected_password};
                const marker = {marker_json};
                const deepActive = (root) => {{
                    let el = root && root.activeElement;
                    while (el) {{
                        if (el.shadowRoot?.activeElement) {{ el = el.shadowRoot.activeElement; continue; }}
                        if (el.tagName === 'IFRAME') {{
                            try {{ const inner = el.contentDocument?.activeElement; if (inner && inner !== el) {{ el = inner; continue; }} }} catch (_) {{}}
                        }}
                        break;
                    }}
                    return el;
                }};
                const target = deepActive(document);
                if (!target || target.__yntraVaultFillMarker !== marker || (target.ownerDocument.defaultView || window).location.origin !== expectedOrigin ||
                    target.disabled || target.readOnly || (target.tagName === 'INPUT' && (target.type === 'password') !== expectedPassword)) return false;
                const view = target.ownerDocument.defaultView || window;
                const setter = target.tagName === 'INPUT'
                    ? Object.getOwnPropertyDescriptor(view.HTMLInputElement.prototype, 'value')?.set
                    : target.tagName === 'TEXTAREA'
                        ? Object.getOwnPropertyDescriptor(view.HTMLTextAreaElement.prototype, 'value')?.set
                        : null;
                if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA') {{
                    if (!setter) return false;
                    setter.call(target, '');
                }} else {{
                    target.textContent = '';
                }}
                target.dispatchEvent(new InputEvent('input', {{bubbles:true, composed:true, inputType:'deleteContentBackward', data:null}}));
                return true;
            }})()
        "#);
        let cleared = page.evaluate(clear_js).await.map_err(|_| {
            crate::error::VaultError::SmartLoginError("Could not clear the verified credential field".into())
        })?;
        if cleared.into_value::<bool>().ok() != Some(true) {
            return Err(crate::error::VaultError::SmartLoginError("Credential field changed before typing".into()));
        }

        for ch in value.chars() {
            if self.is_cancelled() {
                return Err(crate::error::VaultError::SmartLoginError("Login cancelled".into()));
            }
            let ch_json = serde_json::to_string(&ch.to_string()).unwrap_or_else(|_| "\"\"".into());
            let type_js = format!(r#"
                (() => {{
                    const expectedOrigin = {expected_origin};
                    const expectedPassword = {expected_password};
                    const marker = {marker_json};
                    const incoming = {ch_json};
                    const deepActive = (root) => {{
                        let el = root && root.activeElement;
                        while (el) {{
                            if (el.shadowRoot?.activeElement) {{ el = el.shadowRoot.activeElement; continue; }}
                            if (el.tagName === 'IFRAME') {{
                                try {{ const inner = el.contentDocument?.activeElement; if (inner && inner !== el) {{ el = inner; continue; }} }} catch (_) {{}}
                            }}
                            break;
                        }}
                        return el;
                    }};
                    const target = deepActive(document);
                    if (!target || target.__yntraVaultFillMarker !== marker || (target.ownerDocument.defaultView || window).location.origin !== expectedOrigin ||
                        target.disabled || target.readOnly || (target.tagName === 'INPUT' && (target.type === 'password') !== expectedPassword)) return false;
                    const view = target.ownerDocument.defaultView || window;
                    const setter = target.tagName === 'INPUT'
                        ? Object.getOwnPropertyDescriptor(view.HTMLInputElement.prototype, 'value')?.set
                        : target.tagName === 'TEXTAREA'
                            ? Object.getOwnPropertyDescriptor(view.HTMLTextAreaElement.prototype, 'value')?.set
                            : null;
                    if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA') {{
                        if (!setter) return false;
                        setter.call(target, target.value + incoming);
                    }} else {{
                        target.textContent = (target.textContent || '') + incoming;
                    }}
                    target.dispatchEvent(new InputEvent('input', {{bubbles:true, composed:true, inputType:'insertText', data: incoming}}));
                    return true;
                }})()
            "#);
            let typed = page.evaluate(type_js).await.map_err(|_| {
                crate::error::VaultError::SmartLoginError("Could not type into the verified credential field".into())
            })?;
            if typed.into_value::<bool>().ok() != Some(true) {
                return Err(crate::error::VaultError::SmartLoginError("Credential field lost focus".into()));
            }
            let (min_delay, max_delay) = self.config.keystroke_delay_range_ms;
            let delay = if min_delay < max_delay {
                use rand::Rng;
                rand::rng().random_range(min_delay..max_delay)
            } else { min_delay };
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        }

        let finish_js = format!(r#"
            (() => {{
                const expectedOrigin = {expected_origin};
                const marker = {marker_json};
                const deepActive = (root) => {{
                    let el = root && root.activeElement;
                    while (el) {{
                        if (el.shadowRoot?.activeElement) {{ el = el.shadowRoot.activeElement; continue; }}
                        if (el.tagName === 'IFRAME') {{
                            try {{ const inner = el.contentDocument?.activeElement; if (inner && inner !== el) {{ el = inner; continue; }} }} catch (_) {{}}
                        }}
                        break;
                    }}
                    return el;
                }};
                const target = deepActive(document);
                if (!target || target.__yntraVaultFillMarker !== marker || (target.ownerDocument.defaultView || window).location.origin !== expectedOrigin) return false;
                target.dispatchEvent(new Event('change', {{bubbles:true, composed:true}}));
                target.dispatchEvent(new Event('blur', {{bubbles:true, composed:true}}));
                delete target.__yntraVaultFillMarker;
                return true;
            }})()
        "#);
        let finished = page.evaluate(finish_js).await.map_err(|_| {
            crate::error::VaultError::SmartLoginError("Cannot finalize credential input".into())
        })?;
        if finished.into_value::<bool>().ok() != Some(true) {
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
        entry_url: &str,
        value: &Zeroizing<String>,
        is_password: bool,
    ) -> crate::Result<()> {
        use crate::services::autotype::{activate_browser_window, foreground_browser_token, type_browser_field};
        let expected_url = self.verify_fill_origin(page, entry_url).await?;
        let window = match activate_browser_window(&expected_url) {
            Ok(window) => window,
            Err(error) => {
                self.logger.log(
                    LoginState::FillingIdentifier,
                    format!("Browser focus activation did not bind to the verified page: {error}"),
                );
                foreground_browser_token()?
            }
        };
        // Activating the native browser window can move focus to its chrome.
        // Re-run the shared semantic selector after activation so the DOM and
        // UI Automation both point at the same verified credential field.
        let select_js = format!(
            "({})({})",
            include_str!("select_field.js"),
            serde_json::to_string(if is_password { "password" } else { "identifier" }).unwrap()
        );
        let selected = page.evaluate(select_js).await
            .ok()
            .and_then(|value| value.into_value::<String>().ok())
            .is_some_and(|status| status == "found");
        if !selected {
            return Err(crate::error::VaultError::SmartLoginError("Could not focus the verified credential field".into()));
        }
        // CDP remains responsible for selecting the already domain-checked page.
        // Require visible page focus before sending any global OS input.
        let check_js = format!(
            r#"(() => {{
                const deepActive = () => {{
                    let el = document.activeElement;
                    while (el) {{
                        if (el.shadowRoot && el.shadowRoot.activeElement) {{ el = el.shadowRoot.activeElement; continue; }}
                        if (el.tagName === 'IFRAME') {{
                            try {{ const inner = el.contentDocument && el.contentDocument.activeElement; if (inner && inner !== el) {{ el = inner; continue; }} }} catch (_) {{}}
                        }}
                        break;
                    }}
                    return el;
                }};
                const el = deepActive();
                const editable = el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.isContentEditable || el.getAttribute('role') === 'textbox');
                const protectedField = el && el.tagName === 'INPUT' && el.type === 'password';
                if (!document.hasFocus() || !editable || el.disabled || el.readOnly ||
                    protectedField !== {} || el.getClientRects().length === 0) return null;
                return el.tagName === 'INPUT' ? (el.id || '') : '';
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
    async fn direct_submit(&self, page: &Page, entry_url: &str) {
        if self.verify_fill_origin(page, entry_url).await.is_err() { return; }
        #[cfg(target_os = "windows")]
        {
            use crate::services::autotype::{foreground_browser_token, send_enter_guarded, verify_browser_submit};
            if let Ok(token) = foreground_browser_token() {
                if self.is_cancelled() || !page.evaluate(r#"(() => {
                    let el = document.activeElement;
                    while (el && el.shadowRoot?.activeElement) el = el.shadowRoot.activeElement;
                    if (el?.tagName === 'IFRAME') { try { el = el.contentDocument?.activeElement || el; } catch (_) {} }
                    return document.hasFocus() && !!el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.isContentEditable || el.getAttribute('role') === 'textbox');
                })()"#).await
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
            const walk = (root, seen = new Set()) => {
                if (!root || seen.has(root)) return [];
                seen.add(root);
                const nodes = [];
                for (const el of root.querySelectorAll ? root.querySelectorAll('*') : []) {
                    nodes.push(el);
                    if (el.shadowRoot) nodes.push(...walk(el.shadowRoot, seen));
                    if (el.tagName === 'IFRAME') {
                        try { if (el.contentDocument) nodes.push(...walk(el.contentDocument, seen)); } catch (_) {}
                    }
                }
                return nodes;
            };
            const all = walk(document);
            const visible = (el) => {
                if (!el || !el.isConnected || el.closest('[inert]')) return false;
                const style = (el.ownerDocument.defaultView || window).getComputedStyle(el);
                return style.display !== 'none' && style.visibility !== 'hidden' && style.opacity !== '0' &&
                    el.getClientRects().length > 0 && (!el.checkVisibility || el.checkVisibility({checkOpacity:true, checkVisibilityCSS:true}));
            };
            const deepActive = (root) => {
                let el = root && root.activeElement;
                while (el) {
                    if (el.shadowRoot?.activeElement) { el = el.shadowRoot.activeElement; continue; }
                    if (el.tagName === 'IFRAME') {
                        try { const inner = el.contentDocument?.activeElement; if (inner && inner !== el) { el = inner; continue; } } catch (_) {}
                    }
                    break;
                }
                return el;
            };
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
            let focused = deepActive(document);
            if (!focused || focused === document.body || !['INPUT', 'TEXTAREA'].includes(focused.tagName) && !focused.isContentEditable) {
                focused = all.find(el => el.matches('input[autocomplete="one-time-code"], input[name*="otp" i], input[name*="totp" i], input[name*="app_totp" i], input[type="password"]:not([hidden]), input[type="email"]:not([hidden]), input[type="text"]:not([hidden])'));
            }
            const form = focused ? focused.closest('form') : null;
            if (form) {
                // Try form's submit input first
                const submitInput = Array.from(form.querySelectorAll('input[type="submit"]')).find(visible);
                if (submitInput) {
                    submitInput.click();
                    return 'clicked: ' + (submitInput.value || 'submit');
                }
                // Try buttons inside the form
                const formButtons = Array.from(form.querySelectorAll('button, [role="button"]')).filter(visible);
                for (const btn of formButtons) {
                    const match = matchButton(btn);
                    if (match) { btn.click(); return 'clicked: ' + match; }
                }
                // Submit the form directly
                const submitBtn = Array.from(form.querySelectorAll('button[type="submit"]')).find(visible);
                if (submitBtn) { submitBtn.click(); return 'clicked: form submit btn'; }
                if (typeof form.requestSubmit === 'function') {
                    form.requestSubmit();
                } else {
                    form.submit();
                }
                return 'form_submitted';
            }

            // Strategy 2: Page-wide button search (no form context)
            const buttons = all.filter(el => visible(el) && el.matches('button, input[type="submit"], input[type="button"], [role="button"]'));
            for (const btn of buttons) {
                const match = matchButton(btn);
                if (match) { btn.click(); return 'clicked: ' + match; }
            }

            // Strategy 2b: Fallback to any visible submit button on the page
            const submitBtnFallback = all.find(el => visible(el) && el.matches('button[type="submit"]:not([hidden]), input[type="submit"]:not([hidden])'));
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
            const walk = (root, seen = new Set()) => {
                if (!root || seen.has(root)) return [];
                seen.add(root);
                const nodes = [];
                for (const el of root.querySelectorAll ? root.querySelectorAll('*') : []) {
                    nodes.push(el);
                    if (el.shadowRoot) nodes.push(...walk(el.shadowRoot, seen));
                    if (el.tagName === 'IFRAME') {
                        try { if (el.contentDocument) nodes.push(...walk(el.contentDocument, seen)); } catch (_) {}
                    }
                }
                return nodes;
            };
            const patterns = [
                'use another', 'another account', 'add account', 'add an account',
                'lägg till', 'annat konto', 'anderes konto', 'otro cuenta',
                'autre compte', 'use a different',
            ];
            const visible = (el) => {
                if (!el || !el.isConnected || el.closest('[inert]')) return false;
                const style = (el.ownerDocument.defaultView || window).getComputedStyle(el);
                return style.display !== 'none' && style.visibility !== 'hidden' && style.opacity !== '0' &&
                    el.getClientRects().length > 0 && (!el.checkVisibility || el.checkVisibility({checkOpacity:true, checkVisibilityCSS:true}));
            };
            const elements = walk(document).filter(el => el.matches(
                'button, [role="button"], [role="link"], a, li[data-identifier], div[data-identifier]'
            ) && visible(el));
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

#[cfg(test)]
mod identifier_matching_tests {
    use super::*;

    fn input(input_type: &str, name: &str, placeholder: &str, autocomplete: &str) -> InputInfo {
        InputInfo {
            backend_node_id: 0,
            input_type: input_type.into(),
            name: name.into(),
            id: String::new(),
            placeholder: placeholder.into(),
            autocomplete: autocomplete.into(),
            aria_label: String::new(),
            associated_label: String::new(),
            is_visible: true,
            is_readonly: false,
            form_index: None,
            surrounding_text: String::new(),
            ax_role: String::new(),
            ax_name: String::new(),
        }
    }

    #[test]
    fn username_does_not_match_a_phone_first_field() {
        let engine = SmartLoginEngine::new(
            SmartLoginConfig::default(),
            SmartLoginLogger::noop(),
            Arc::new(AtomicBool::new(false)),
        );
        let phone = input("text", "mobile", "Phone number", "username webauthn");
        assert!(!engine.identifier_field_matches(&phone, "demo_user"));
    }

    #[test]
    fn generic_identifier_and_username_fields_match_the_saved_username() {
        let engine = SmartLoginEngine::new(
            SmartLoginConfig::default(),
            SmartLoginLogger::noop(),
            Arc::new(AtomicBool::new(false)),
        );
        let generic = input("text", "identifier", "Email or username", "username");
        assert!(engine.identifier_field_matches(&generic, "demo_user"));
    }
}
