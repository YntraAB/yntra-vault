//! Site-independent outcome policy, shared by native accessibility and CDP observers.
//! Observers supply current evidence; neither a delay nor a missing field means success.
#[derive(Default)]
pub(crate) struct Evidence {
    pub trusted_site: bool,
    pub credential_form: bool,
    pub invalid_credentials: bool,
    pub captcha: bool,
    pub mfa: bool,
    pub locked: bool,
    pub blocked: bool,
    pub authenticated_control: bool,
    pub other_account: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Unconfirmed,
    Authenticated,
    DifferentAccount,
    InvalidCredentials,
    Captcha,
    Mfa,
    Locked,
    Blocked,
}

pub(crate) fn assess(e: &Evidence) -> Outcome {
    if !e.trusted_site {
        return Outcome::Unconfirmed;
    }
    if e.blocked {
        return Outcome::Blocked;
    }
    if e.locked {
        return Outcome::Locked;
    }
    if e.captcha {
        return Outcome::Captcha;
    }
    if e.invalid_credentials {
        return Outcome::InvalidCredentials;
    }
    if e.mfa {
        return Outcome::Mfa;
    }
    if !e.credential_form {
        if e.authenticated_control && e.other_account {
            return Outcome::Unconfirmed;
        }
        if e.other_account {
            return Outcome::DifferentAccount;
        }
        if e.authenticated_control {
            return Outcome::Authenticated;
        }
    }
    Outcome::Unconfirmed
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_fields_or_untrusted_redirect_never_prove_success() {
        assert_eq!(
            assess(&Evidence {
                trusted_site: true,
                ..Default::default()
            }),
            Outcome::Unconfirmed
        );
        assert_eq!(
            assess(&Evidence {
                authenticated_control: true,
                ..Default::default()
            }),
            Outcome::Unconfirmed
        );
    }
    #[test]
    fn errors_and_challenges_win_over_authenticated_markers() {
        let mut e = Evidence {
            trusted_site: true,
            authenticated_control: true,
            ..Default::default()
        };
        assert_eq!(assess(&e), Outcome::Authenticated);
        e.credential_form = true;
        assert_eq!(assess(&e), Outcome::Unconfirmed);
        e.invalid_credentials = true;
        assert_eq!(assess(&e), Outcome::InvalidCredentials);
        e.captcha = true;
        assert_eq!(assess(&e), Outcome::Captcha);
        e.locked = true;
        assert_eq!(assess(&e), Outcome::Locked);
    }
    #[test]
    fn ambiguous_account_menus_cannot_confirm_identity() {
        let mut e = Evidence {
            trusted_site: true,
            other_account: true,
            ..Default::default()
        };
        assert_eq!(assess(&e), Outcome::DifferentAccount);
        e.authenticated_control = true;
        assert_eq!(assess(&e), Outcome::Unconfirmed);
    }
}
