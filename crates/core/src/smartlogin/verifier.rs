//! State Verifier — checks the outcome after form submission.
//! Uses adaptive waiting that polls for actual page state changes
//! instead of fixed delays.

use chromiumoxide::page::Page;

use crate::smartlogin::types::*;
use crate::smartlogin::logging::{SmartLoginLogger, SmartLoginEvent, SmartLoginEventDetail};

/// Lightweight JS that captures a page state fingerprint.
/// Used for change detection (fast — runs in <1ms).
const SNAPSHOT_JS: &str = r#"
(() => {
    const has_mfa = document.querySelector('input[autocomplete="one-time-code"], input[name*="otp" i], input[name*="totp" i], input[name*="mfa" i], input[name*="2fa" i], input[name*="code" i], input[name*="pin" i], input[name*="otc" i], input[name*="two_factor" i], input[name*="app_totp" i], input[inputmode="numeric"]') !== null;
    const has_error = document.querySelector('[class*="error"], [role="alert"], .flash-error, .error-message') !== null;
    return JSON.stringify({
        url: window.location.href,
        has_pw: Array.from(document.querySelectorAll('input[type="password"]')).some(el => el.offsetParent !== null),
        has_mfa: has_mfa,
        has_error: has_error,
        forms: document.forms.length,
        ready: document.readyState,
    });
})()
"#;

/// Full verification JS — only runs once the page has stabilized.
const VERIFY_JS: &str = r#"
(() => {
    const result = {
        has_password_input: false,
        has_error_message: false,
        error_text: '',
        has_captcha: false,
        captcha_type: '',
        has_mfa_input: false,
        has_mfa_text: false,
        mfa_type: '',
        has_account_locked: false,
        locked_text: '',
        url: window.location.href,
        title: document.title || '',
    };

    result.has_password_input = Array.from(document.querySelectorAll('input[type="password"]')).some(el => el.offsetParent !== null);

    // Error messages — only in alert/error containers
    const errorPatterns = [
        'incorrect', 'invalid', 'wrong', 'failed', 'denied',
        'does not match', 'not found', 'not recognized', 'try again',
        'fel', 'felaktig', 'ogiltig', 'falsch', 'ungültig',
        'incorrecto', 'inválido', 'erreur', 'erroné',
    ];
    const errorSelectors = [
        '[class*="error"]', '[role="alert"]', '[aria-live="assertive"]',
        '.flash-error', '.error-message', '.login-error',
    ];
    for (const selector of errorSelectors) {
        const els = document.querySelectorAll(selector);
        for (const el of els) {
            const text = (el.textContent || '').trim().toLowerCase();
            if (text.length > 3 && text.length < 300) {
                for (const pattern of errorPatterns) {
                    if (text.includes(pattern)) {
                        result.has_error_message = true;
                        result.error_text = el.textContent.trim().substring(0, 200);
                        break;
                    }
                }
            }
            if (result.has_error_message) break;
        }
        if (result.has_error_message) break;
    }

    // CAPTCHA
    const captchaSelectors = [
        'iframe[src*="recaptcha"]', 'iframe[src*="hcaptcha"]',
        'iframe[src*="turnstile"]', 'iframe[src*="captcha"]',
        '.g-recaptcha', '.h-captcha', '[data-sitekey]',
        '#captcha', '.captcha',
    ];
    for (const sel of captchaSelectors) {
        if (document.querySelector(sel)) {
            result.has_captcha = true;
            if (sel.includes('recaptcha')) result.captcha_type = 'reCAPTCHA';
            else if (sel.includes('hcaptcha')) result.captcha_type = 'hCaptcha';
            else if (sel.includes('turnstile')) result.captcha_type = 'Cloudflare Turnstile';
            else result.captcha_type = 'CAPTCHA';
            break;
        }
    }

    // MFA — detect OTP input fields or 2FA page markers
    const otpInputs = document.querySelectorAll(
        'input[autocomplete="one-time-code"], input[name*="otp" i], input[name*="totp" i], input[name*="mfa" i], input[name*="2fa" i], input[name*="code" i], input[name*="pin" i], input[name*="otc" i], input[name*="two_factor" i], input[name*="app_totp" i], input[name*="token" i], input[id*="otp" i], input[id*="totp" i], input[id*="mfa" i], input[id*="2fa" i], input[id*="code" i], input[id*="pin" i], input[id*="otc" i], input[id*="app_totp" i], input[inputmode="numeric"]'
    );
    if (otpInputs.length > 0) {
        result.has_mfa_input = true;
        result.mfa_type = 'OTP code';
    }

    // Secondary MFA signal (only when password form is gone)
    if (result.has_mfa_input || !result.has_password_input) {
        const mfaPatterns = [
            'verification code', 'verifikationskod', 'authenticator', 'two-factor', '2fa',
            'two-step', '2-step', 'tvåfaktor', 'tvåstegs', 'one-time code', 'enter the code',
            'ange koden', 'security code', 'säkerhetskod', 'passcode', 'auth code',
            'zweifaktor', 'código de verificación', 'code de vérification', 'verificatiecode',
        ];
        const bodyText = document.body ? document.body.innerText.substring(0, 3000).toLowerCase() : '';
        for (const pattern of mfaPatterns) {
            if (bodyText.includes(pattern)) {
                result.has_mfa_text = true;
                if (!result.mfa_type) result.mfa_type = 'authenticator';
                break;
            }
        }
    }

    // Account lock
    const lockPatterns = [
        'account locked', 'too many attempts', 'temporarily blocked',
        'account suspended', 'account disabled',
    ];
    const bodyLower = document.body ? document.body.innerText.substring(0, 2000).toLowerCase() : '';
    for (const pattern of lockPatterns) {
        if (bodyLower.includes(pattern)) {
            result.has_account_locked = true;
            result.locked_text = pattern;
            break;
        }
    }

    return JSON.stringify(result);
})()
"#;

#[derive(serde::Deserialize)]
struct PageSnapshot {
    url: String,
    has_pw: bool,
    has_mfa: bool,
    has_error: bool,
    forms: usize,
    ready: String,
}

#[derive(serde::Deserialize)]
struct VerifyResult {
    has_password_input: bool,
    has_error_message: bool,
    error_text: String,
    has_captcha: bool,
    captcha_type: String,
    has_mfa_input: bool,
    has_mfa_text: bool,
    mfa_type: String,
    has_account_locked: bool,
    locked_text: String,
    url: String,
    #[allow(dead_code)]
    title: String,
}

/// Verify the login result after form submission.
/// Uses adaptive polling instead of fixed delays — waits for actual
/// page state changes (URL change, form disappearance, or error appearance).
pub async fn verify_login_result(
    page: &Page,
    _original_url: &str,
    _entry_url: &str,
    logger: &SmartLoginLogger,
) -> crate::Result<LoginResult> {
    logger.log(LoginState::VerifyingResult, "Waiting for page response...");

    // Take a snapshot of the current page state before waiting
    let before = take_snapshot(page).await;

    // Adaptive wait: poll every 150ms for up to 8 seconds
    // Exit early as soon as URL changes or password field disappears
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(8);
    let poll_interval = tokio::time::Duration::from_millis(150);
    let mut settled_count: u32 = 0;

    loop {
        tokio::time::sleep(poll_interval).await;

        if tokio::time::Instant::now() > deadline {
            logger.log(LoginState::VerifyingResult, "Timeout — analyzing current state");
            break;
        }

        let current = take_snapshot(page).await;

        // Detect changes from the pre-submit state
        let url_changed = current.url != before.url;
        let pw_gone = before.has_pw && !current.has_pw;
        let mfa_appeared = !before.has_mfa && current.has_mfa;
        let error_appeared = !before.has_error && current.has_error;
        let form_changed = current.forms != before.forms;

        if url_changed || pw_gone || mfa_appeared || error_appeared || form_changed {
            // Something changed — wait 1 more poll to let the page settle
            settled_count += 1;
            if settled_count >= 1 {
                logger.log(LoginState::VerifyingResult, "Page settled — verifying");
                break;
            }
        }

        // If readyState is 'loading', the page is still navigating
        if current.ready == "complete" && settled_count > 0 {
            break;
        }
    }

    // Now run the full verification
    logger.log(LoginState::VerifyingResult, "Analyzing login result...");

    let result = page
        .evaluate(VERIFY_JS)
        .await
        .map_err(|e| crate::error::VaultError::SmartLoginError(
            format!("Verification script failed: {e}"),
        ))?;

    let json_str: String = result.into_value().map_err(|e| {
        crate::error::VaultError::SmartLoginError(format!("Failed to parse verify result: {e}"))
    })?;

    let verify: VerifyResult = serde_json::from_str(&json_str).map_err(|e| {
        crate::error::VaultError::SmartLoginError(format!("Invalid verify JSON: {e}"))
    })?;

    // Priority 1: CAPTCHA
    if verify.has_captcha {
        logger.emit(
            SmartLoginEvent::new(
                LoginState::RequiresManualAction(ManualActionType::Captcha),
                format!("{} detected — complete it in the browser", verify.captcha_type),
            )
            .with_detail(SmartLoginEventDetail::Verification {
                result: format!("CAPTCHA: {}", verify.captcha_type),
            }),
        );
        return Ok(LoginResult::RequiresCaptcha);
    }

    // Priority 2: Account lock
    if verify.has_account_locked {
        logger.emit(
            SmartLoginEvent::new(
                LoginState::Failed("Account locked".into()),
                "Account is locked or too many attempts",
            )
            .with_detail(SmartLoginEventDetail::Verification {
                result: format!("Locked: {}", verify.locked_text),
            }),
        );
        return Ok(LoginResult::AccountLocked {
            message: verify.locked_text,
        });
    }

    // Priority 3: Credential errors (only if form is still visible)
    if verify.has_error_message && verify.has_password_input {
        logger.emit(
            SmartLoginEvent::new(
                LoginState::Failed("Wrong credentials".into()),
                format!("Login error: {}", verify.error_text),
            )
            .with_detail(SmartLoginEventDetail::Verification {
                result: "wrong_credentials".into(),
            }),
        );
        return Ok(LoginResult::WrongCredentials {
            error_message: Some(verify.error_text),
        });
    }

    // Priority 4: Success vs MFA (triggers MFA if either OTP input OR 2FA text is detected)
    if !verify.has_password_input && !verify.has_error_message {
        if verify.has_mfa_input || verify.has_mfa_text {
            let mfa_desc = if !verify.mfa_type.is_empty() { verify.mfa_type.clone() } else { "2FA / Authenticator".into() };
            logger.emit(
                SmartLoginEvent::new(
                    LoginState::RequiresManualAction(ManualActionType::TwoFactorAuth),
                    format!("Two-factor authentication required ({})", mfa_desc),
                )
                .with_detail(SmartLoginEventDetail::Verification {
                    result: format!("MFA: {}", mfa_desc),
                }),
            );
            return Ok(LoginResult::RequiresMfa {
                mfa_type: mfa_desc,
            });
        }

        logger.emit(
            SmartLoginEvent::new(LoginState::Success, format!("Login successful ({})", verify.url))
                .with_detail(SmartLoginEventDetail::Verification {
                    result: "success".into(),
                }),
        );
        return Ok(LoginResult::Success {
            final_url: verify.url,
        });
    }

    // Form still present, no error — unclear
    logger.emit(
        SmartLoginEvent::new(
            LoginState::VerifyingResult,
            "Login form still present — result unclear",
        )
        .with_detail(SmartLoginEventDetail::Verification {
            result: "uncertain".into(),
        }),
    );

    Ok(LoginResult::UnexpectedState {
        description: "Login form is still visible after submission".into(),
    })
}

/// Take a lightweight snapshot of the page state for change detection.
async fn take_snapshot(page: &Page) -> PageSnapshot {
    let default = PageSnapshot {
        url: String::new(),
        has_pw: false,
        has_mfa: false,
        has_error: false,
        forms: 0,
        ready: "unknown".into(),
    };

    let Ok(result) = page.evaluate(SNAPSHOT_JS).await else {
        return default;
    };

    let Ok(json_str) = result.into_value::<String>() else {
        return default;
    };

    serde_json::from_str(&json_str).unwrap_or(default)
}
