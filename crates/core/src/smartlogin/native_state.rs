//! Conservative, testable interpretation of ordinary-browser accessibility signals.
//! No cookies, tokens, form values or Google private APIs are required.
#[derive(Default)]
pub(crate) struct Signals {
    pub url: String,
    pub identifier: bool,
    pub password: bool,
    pub invalid: bool,
    pub captcha: bool,
    pub mfa: bool,
    pub locked: bool,
    pub blocked: bool,
    pub account_matches: bool,
    pub other_account: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PageState {
    Unknown,
    Identifier,
    Password,
    Captcha,
    Mfa,
    InvalidCredentials,
    Locked,
    Blocked,
    Authenticated,
    OtherAccount,
}

pub(crate) fn account_host(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            && u.host_str() == Some("accounts.google.com")
            && u.username().is_empty()
            && u.password().is_none()
    })
}

pub(crate) fn service_host(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            && u.username().is_empty()
            && u.password().is_none()
            && match u.host_str() {
                Some("mail.google.com") => u.path().starts_with("/mail/"),
                Some("myaccount.google.com") => true,
                Some(
                    "drive.google.com"
                    | "docs.google.com"
                    | "calendar.google.com"
                    | "www.google.com"
                    | "google.com",
                ) => true,
                _ => false,
            }
    })
}

pub(crate) fn landing_url(entry: &str) -> String {
    let normalized = if entry.contains("://") {
        entry.to_owned()
    } else {
        format!("https://{entry}")
    };
    let Ok(url) = reqwest::Url::parse(&normalized) else {
        return "https://mail.google.com/mail/".into();
    };
    match url.host_str() {
        Some("gmail.com" | "www.gmail.com" | "accounts.google.com" | "mail.google.com") => {
            "https://mail.google.com/mail/".into()
        }
        _ if service_host(url.as_str()) => normalized,
        _ => "https://myaccount.google.com/".into(),
    }
}

/// Account selection must not prefill credentials or skip the keyboard step.
/// The chooser can reuse an existing session; AddSession opens a blank login.
pub(crate) fn account_navigation(entry: &str, add_session: bool) -> String {
    let mut destination = reqwest::Url::parse(&landing_url(entry)).expect("validated landing URL");
    // Saved /u/0 routes and authuser parameters refer to the browser's account order,
    // not the selected vault entry. Do not carry those selectors into this attempt.
    let segments: Vec<_> = destination.path().split('/').collect();
    let mut path = Vec::new();
    let mut index = 0;
    while index < segments.len() {
        if segments[index] == "u"
            && segments
                .get(index + 1)
                .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        {
            index += 2;
        } else {
            path.push(segments[index]);
            index += 1;
        }
    }
    let path = path.join("/");
    destination.set_path(&path);
    let query: Vec<_> = destination
        .query_pairs()
        .filter(|(key, _)| {
            !["authuser", "email", "login_hint"].contains(&key.to_ascii_lowercase().as_str())
        })
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    destination.set_query(None);
    if !query.is_empty() {
        destination.query_pairs_mut().extend_pairs(query);
    }
    let mut url = reqwest::Url::parse(if add_session {
        "https://accounts.google.com/AddSession"
    } else {
        "https://accounts.google.com/AccountChooser"
    })
    .unwrap();
    url.query_pairs_mut()
        .append_pair("continue", destination.as_str());
    url.into()
}

/// Another session is a navigation condition before input, not a login failure.
/// Bound the fallback and never resubmit credentials after a completed attempt.
pub(crate) fn can_add_account(add_session_started: bool, credentials_started: bool) -> bool {
    !add_session_started && !credentials_started
}

pub(crate) fn password_account_verified(signals: &Signals, identifier_sent: bool) -> bool {
    !signals.other_account && (signals.account_matches || identifier_sent)
}

pub(crate) fn account_chooser(url: &str) -> bool {
    account_host(url)
        && reqwest::Url::parse(url).is_ok_and(|u| {
            let path = u.path().trim_end_matches('/').to_ascii_lowercase();
            path == "/accountchooser" || path.ends_with("/signin/accountchooser")
        })
}

/// Wait for observed list stability, not an unconditional three-second pause.
#[derive(Default)]
pub(crate) struct ChooserWait {
    started: Option<std::time::Duration>,
    unchanged: Option<(u64, std::time::Duration)>,
}

impl ChooserWait {
    pub fn ready_for_new_account(
        &mut self,
        now: std::time::Duration,
        missing_from_list: Option<u64>,
    ) -> bool {
        let started = *self.started.get_or_insert(now);
        match (self.unchanged, missing_from_list) {
            (Some((old, since)), Some(current)) if old == current => {
                if now.saturating_sub(since) >= std::time::Duration::from_millis(250) {
                    return true;
                }
            }
            (_, Some(current)) => self.unchanged = Some((current, now)),
            (_, None) => self.unchanged = None,
        }
        now.saturating_sub(started) >= std::time::Duration::from_secs(3)
    }
}

pub(crate) fn email_control(name: &str, identifier: &str) -> Option<bool> {
    if !identifier.contains('@') {
        return None;
    }
    let lower = name.to_lowercase();
    let emails: Vec<_> = lower
        .split(|c: char| !(c.is_alphanumeric() || "@._+-".contains(c)))
        .filter(|part| part.contains('@'))
        .collect();
    if emails.len() != 1 {
        return None;
    }
    Some(emails[0] == identifier.trim().to_lowercase())
}

pub(crate) fn classify(s: &Signals) -> PageState {
    use super::outcome::{Evidence, Outcome, assess};
    let authentication = account_host(&s.url);
    let service = service_host(&s.url);
    let outcome = assess(&Evidence {
        trusted_site: authentication || service,
        credential_form: s.identifier || s.password,
        invalid_credentials: authentication && s.invalid,
        captcha: authentication && s.captcha,
        mfa: authentication && s.mfa,
        locked: authentication && s.locked,
        blocked: authentication && s.blocked,
        authenticated_control: service && s.account_matches,
        other_account: service && s.other_account,
    });
    match outcome {
        Outcome::Authenticated => return PageState::Authenticated,
        Outcome::DifferentAccount => return PageState::OtherAccount,
        Outcome::InvalidCredentials => return PageState::InvalidCredentials,
        Outcome::Captcha => return PageState::Captcha,
        Outcome::Mfa => return PageState::Mfa,
        Outcome::Locked => return PageState::Locked,
        Outcome::Blocked => return PageState::Blocked,
        Outcome::Unconfirmed => {}
    }
    if account_host(&s.url) {
        if s.password {
            return PageState::Password;
        }
        if s.identifier {
            return PageState::Identifier;
        }
    }
    PageState::Unknown
}

/// Only Google account controls qualify; arbitrary page text is not identity evidence.
pub(crate) fn account_control(name: &str, identifier: &str) -> Option<bool> {
    let lower = name.to_lowercase();
    let account_label = lower.contains("google")
        && [
            "account",
            "konto",
            "compte",
            "cuenta",
            "conta",
            "cont Google",
            "аккаунт",
            "обліков",
            "حساب",
            "חשבון",
            "アカウント",
            "계정",
            "帐号",
            "账号",
            "帳戶",
            "hesab",
            "खाता",
            "λογαριασ",
            "fiók",
            "účet",
            "tili",
        ]
        .iter()
        .any(|label| lower.contains(label));
    if !account_label {
        return None;
    }
    email_control(name, identifier)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chooser_moves_on_when_a_visible_list_settles_not_after_three_seconds() {
        use std::time::Duration;
        let mut wait = ChooserWait::default();
        assert!(!wait.ready_for_new_account(Duration::ZERO, Some(1)));
        assert!(!wait.ready_for_new_account(Duration::from_millis(100), Some(1)));
        assert!(wait.ready_for_new_account(Duration::from_millis(250), Some(1)));
    }
    #[test]
    fn changing_or_disappearing_account_lists_do_not_count_as_stable() {
        use std::time::Duration;
        let mut wait = ChooserWait::default();
        assert!(!wait.ready_for_new_account(Duration::ZERO, Some(1)));
        assert!(!wait.ready_for_new_account(Duration::from_millis(200), Some(2)));
        assert!(!wait.ready_for_new_account(Duration::from_millis(350), Some(2)));
        assert!(!wait.ready_for_new_account(Duration::from_millis(400), None));
        assert!(!wait.ready_for_new_account(Duration::from_millis(500), Some(2)));
        assert!(wait.ready_for_new_account(Duration::from_millis(750), Some(2)));
    }
    #[test]
    fn unknown_chooser_keeps_a_bounded_loading_grace_period() {
        use std::time::Duration;
        let mut wait = ChooserWait::default();
        assert!(!wait.ready_for_new_account(Duration::ZERO, None));
        assert!(!wait.ready_for_new_account(Duration::from_millis(2999), None));
        assert!(wait.ready_for_new_account(Duration::from_secs(3), None));
    }
    #[test]
    fn chooser_does_not_prefill_or_skip_identifier_input() {
        let url = reqwest::Url::parse(&account_navigation("https://gmail.com", false)).unwrap();
        assert_eq!(url.host_str(), Some("accounts.google.com"));
        assert_eq!(url.path(), "/AccountChooser");
        let pairs: std::collections::HashMap<_, _> = url.query_pairs().collect();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs["continue"], "https://mail.google.com/mail/");
        assert!(!pairs.contains_key("prompt"));
    }
    #[test]
    fn add_account_is_bounded_and_never_retries_submitted_credentials() {
        assert!(can_add_account(false, false));
        assert!(!can_add_account(true, false));
        assert!(!can_add_account(false, true));
        assert!(!can_add_account(true, true));
        let url = reqwest::Url::parse(&account_navigation(
            "https://drive.google.com/drive/u/0/my-drive?authuser=0&Email=old%40example.test&login_hint=old",
            true,
        ))
        .unwrap();
        assert_eq!(url.path(), "/AddSession");
        let pairs: std::collections::HashMap<_, _> = url.query_pairs().collect();
        assert_eq!(pairs["continue"], "https://drive.google.com/drive/my-drive");
        assert_eq!(pairs.len(), 1);
    }
    #[test]
    fn navigation_rejects_untrusted_continuations() {
        let url = reqwest::Url::parse(&account_navigation("https://evil.test/", false)).unwrap();
        assert!(account_host(url.as_str()));
        let pairs: std::collections::HashMap<_, _> = url.query_pairs().collect();
        assert_eq!(pairs.len(), 1);
        assert!(service_host(&pairs["continue"]));
    }
    #[test]
    fn password_account_chip_requires_one_exact_identity() {
        assert_eq!(
            email_control("Switch account SECOND@example.test", "second@example.test"),
            Some(true)
        );
        assert_eq!(
            email_control("first@example.test", "second@example.test"),
            Some(false)
        );
        assert_eq!(
            email_control(
                "first@example.test second@example.test",
                "second@example.test"
            ),
            None
        );
        assert_eq!(email_control("Select account", "second@example.test"), None);
    }
    #[test]
    fn direct_password_step_requires_selected_identity_and_rejects_mismatches() {
        let mut signals = Signals::default();
        assert!(!password_account_verified(&signals, false));
        assert!(password_account_verified(&signals, true));
        signals.account_matches = true;
        assert!(password_account_verified(&signals, false));
        signals.other_account = true;
        assert!(!password_account_verified(&signals, false));
        assert!(!password_account_verified(&signals, true));
        assert!(account_chooser(
            "https://accounts.google.com/AccountChooser?Email=demo%40example.test"
        ));
        assert!(account_chooser(
            "https://accounts.google.com/v3/signin/accountchooser?service=mail"
        ));
        assert!(!account_chooser(
            "https://accounts.google.com.evil.test/AccountChooser"
        ));
        assert!(!account_chooser(
            "https://accounts.google.com/signin/challenge/pwd"
        ));
    }
    #[test]
    fn success_needs_trusted_service_and_matching_identity() {
        let mut s = Signals {
            url: "https://mail.google.com/mail/u/0/".into(),
            ..Default::default()
        };
        assert_eq!(classify(&s), PageState::Unknown);
        s.other_account = true;
        assert_eq!(classify(&s), PageState::OtherAccount);
        s.account_matches = true;
        assert_eq!(classify(&s), PageState::Unknown);
        s.other_account = false;
        assert_eq!(classify(&s), PageState::Authenticated);
        s.password = true;
        assert_eq!(classify(&s), PageState::Unknown);
        s.password = false;
        s.url = "https://mail.google.com.evil.test/mail/".into();
        assert_eq!(classify(&s), PageState::Unknown);
    }
    #[test]
    fn challenges_and_errors_override_fields() {
        let mut s = Signals {
            url: "https://accounts.google.com/v3/signin/challenge/pwd".into(),
            password: true,
            ..Default::default()
        };
        assert_eq!(classify(&s), PageState::Password);
        s.invalid = true;
        assert_eq!(classify(&s), PageState::InvalidCredentials);
        s.captcha = true;
        assert_eq!(classify(&s), PageState::Captcha);
        s.locked = true;
        assert_eq!(classify(&s), PageState::Locked);
        s.blocked = true;
        assert_eq!(classify(&s), PageState::Blocked);
        s.url = "http://accounts.google.com/".into();
        assert_eq!(classify(&s), PageState::Unknown);
    }
    #[test]
    fn empty_loading_pages_never_mean_success() {
        assert_eq!(classify(&Signals::default()), PageState::Unknown);
        assert_eq!(
            classify(&Signals {
                url: "https://accounts.google.com/".into(),
                ..Default::default()
            }),
            PageState::Unknown
        );
    }
    #[test]
    fn account_identity_requires_an_exact_email_token_and_account_control() {
        assert_eq!(
            account_control(
                "Google Account: Test (test@example.com)",
                "TEST@example.com"
            ),
            Some(true)
        );
        assert_eq!(
            account_control("Google-konto: Test (test@example.com)", "test@example.com"),
            Some(true)
        );
        assert_eq!(
            account_control(
                "Google Account: (othertest@example.com)",
                "test@example.com"
            ),
            Some(false)
        );
        assert_eq!(
            account_control("Message to test@example.com", "test@example.com"),
            None
        );
        assert_eq!(account_control("Google Account", "test@example.com"), None);
    }
    #[test]
    fn landing_does_not_force_add_session() {
        assert_eq!(
            landing_url("https://gmail.com"),
            "https://mail.google.com/mail/"
        );
        assert_eq!(
            landing_url("https://accounts.google.com/AddSession"),
            "https://mail.google.com/mail/"
        );
        assert_eq!(
            landing_url("https://drive.google.com/drive/u/0"),
            "https://drive.google.com/drive/u/0"
        );
    }
}
