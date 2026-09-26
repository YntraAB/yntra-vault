//! Evidence-based session/result checks. Loading pages are retried within a bound.
use chromiumoxide::page::Page;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::smartlogin::types::*;
use crate::smartlogin::logging::SmartLoginLogger;
use super::outcome::{assess, Evidence, Outcome};

const VERIFY_JS: &str = include_str!("verify_page.js");

fn verification_script(identifier: &str) -> String {
    VERIFY_JS.replace("__EXPECTED_IDENTIFIER__", &serde_json::to_string(&identifier.trim().to_lowercase()).unwrap_or_else(|_| "\"\"".into()))
}

#[derive(serde::Deserialize)]
struct VerifyResult {
    authenticated_control: bool,
    identity_match: bool,
    identity_mismatch: bool,
    identity_ambiguous: bool,
    has_identifier_input: bool,
    has_password_input: bool,
    has_error_message: bool,
    has_captcha: bool,
    has_mfa_input: bool,
    has_mfa_text: bool,
    mfa_type: String,
    has_account_locked: bool,
    url: String,
    ready: String,
}

impl VerifyResult {
    fn outcome(&self) -> Outcome {
        assess(&Evidence {
            trusted_site: true,
            credential_form: self.has_password_input || self.has_identifier_input,
            invalid_credentials: self.has_error_message,
            captcha: self.has_captcha,
            mfa: self.has_mfa_input && self.has_mfa_text,
            locked: self.has_account_locked,
            authenticated_control: self.authenticated_control && !self.identity_ambiguous && !self.identity_mismatch,
            other_account: self.authenticated_control && self.identity_mismatch,
            ..Default::default()
        })
    }
}

async fn observe(page: &Page, script: &str) -> Option<VerifyResult> {
    let result = page.evaluate(script).await.ok()?.into_value::<String>().ok()?;
    serde_json::from_str(&result).ok()
}

fn trusted_result(entry_url: &str, result_url: &str) -> bool {
    reqwest::Url::parse(result_url).is_ok_and(|url| {
        (url.scheme() == "https" || (url.scheme() == "http" && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))))
            && url.username().is_empty() && url.password().is_none()
    }) && crate::smartlogin::discovery::is_allowed_auth_domain(entry_url, result_url)
}

fn public_url(url: &str) -> Option<String> {
    let mut url = reqwest::Url::parse(url).ok()?;
    url.set_query(None);
    url.set_fragment(None);
    Some(url.into())
}

/// A recognized session blocks discovery even when its account cannot be matched.
/// Only an exact identity match is reported as AlreadySignedIn.
pub(crate) async fn existing_session(page: &Page, entry_url: &str, identifier: &str) -> Option<LoginResult> {
    let state = observe(page, &verification_script(identifier)).await?;
    if state.ready == "loading" || !trusted_result(entry_url, &state.url)
        || state.has_password_input || state.has_identifier_input || state.has_error_message
        || state.has_captcha || state.has_mfa_input || state.has_account_locked
        || !state.authenticated_control { return None; }
    Some(if state.identity_mismatch {
        LoginResult::DifferentAccount
    } else if state.identity_match && !state.identity_ambiguous {
        LoginResult::AlreadySignedIn { final_url: public_url(&state.url)? }
    } else {
        LoginResult::RequiresManualAction
    })
}

/// Observe until actual evidence settles, including delayed redirects after TOTP.
/// A transient JS execution-context error or a first Unconfirmed frame is not final.
pub async fn verify_login_result(
    page: &Page,
    original_url: &str,
    entry_url: &str,
    identifier: &str,
    after_mfa: bool,
    cancel: &AtomicBool,
    logger: &SmartLoginLogger,
) -> crate::Result<LoginResult> {
    logger.log(LoginState::VerifyingResult, "Waiting for confirmed browser state...");
    let script = verification_script(identifier);
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(8);
    let mut stable: Option<(String, String)> = None;
    let mut last;
    loop {
        if cancel.load(Ordering::Relaxed) { return Ok(LoginResult::Cancelled); }
        if let Some(state) = observe(page, &script).await {
            if cancel.load(Ordering::Relaxed) { return Ok(LoginResult::Cancelled); }
            if !trusted_result(entry_url, &state.url) {
                // A loading navigation is not yet a final redirect destination.
                if state.ready != "loading" && state.url.starts_with("https://") {
                    return Ok(LoginResult::DomainMismatch { expected: "Saved website".into(), actual: "Unexpected website".into() });
                }
                stable = None;
                last = None;
            } else {
                let outcome = state.outcome();
                let key = (state.url.clone(), format!("{outcome:?}"));
                let pending_old_mfa = after_mfa && outcome == Outcome::Mfa && state.url == original_url;
                if state.ready != "loading" && outcome != Outcome::Unconfirmed && !pending_old_mfa {
                    if stable.as_ref() == Some(&key) {
                        return finish(state, logger);
                    }
                    stable = Some(key);
                } else { stable = None; }
                last = Some(state);
            }
        } else {
            stable = None;
            last = None;
        }
        if tokio::time::Instant::now() >= deadline { break; }
        tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
    }
    // At the bound only persistent non-success evidence is useful. A single late
    // authenticated frame must still not be promoted to a verified success.
    if let Some(state) = last {
        let outcome = state.outcome();
        if state.ready != "loading" && outcome != Outcome::Authenticated && outcome != Outcome::Unconfirmed {
            return finish(state, logger);
        }
    }
    logger.log(LoginState::VerifyingResult, "Browser state remains unconfirmed after observation");
    Ok(LoginResult::RequiresManualAction)
}

fn finish(verify: VerifyResult, logger: &SmartLoginLogger) -> crate::Result<LoginResult> {
    let outcome = verify.outcome();
    let result = match outcome {
        Outcome::Captcha => LoginResult::RequiresCaptcha,
        Outcome::Mfa => LoginResult::RequiresMfa { mfa_type: verify.mfa_type },
        Outcome::Locked => LoginResult::AccountLocked { message: "The website restricted sign-in".into() },
        Outcome::InvalidCredentials => LoginResult::WrongCredentials { error_message: None },
        Outcome::DifferentAccount => LoginResult::DifferentAccount,
        Outcome::Authenticated => LoginResult::Success {
            final_url: public_url(&verify.url).ok_or_else(|| crate::error::VaultError::SmartLoginError("Invalid result address".into()))?
        },
        _ => LoginResult::RequiresManualAction,
    };
    logger.log(if matches!(result, LoginResult::Success { .. }) { LoginState::Success } else { LoginState::VerifyingResult },
        format!("Browser result: {outcome:?}"));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn result_host_must_be_trusted_and_have_no_url_userinfo() {
        assert!(trusted_result("https://github.com", "https://github.com/"));
        assert!(!trusted_result("https://github.com", "https://github.com.evil.test/"));
        assert!(!trusted_result("https://github.com", "https://user@github.com/"));
        assert!(!trusted_result("https://github.com", "http://github.com/"));
    }
    #[test]
    fn identifier_is_encoded_as_data_and_result_urls_drop_private_parameters() {
        assert!(verification_script("name\";alert(1)//").contains("name\\\";alert(1)//"));
        assert_eq!(public_url("https://github.com/?private=value#fragment").as_deref(), Some("https://github.com/"));
    }
}
