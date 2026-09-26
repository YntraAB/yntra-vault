use super::*;
use crate::smartlogin::native_state::{Signals, account_control, account_host};
use windows::Win32::UI::Accessibility::{UIA_CheckBoxControlTypeId, UIA_TextControlTypeId};

pub(super) fn observe(
    automation: &IUIAutomation,
    window: &IUIAutomationElement,
    address: String,
    identifier: &str,
) -> Signals {
    let authentication = account_host(&address);
    let mut state = Signals {
        url: address,
        ..Default::default()
    };
    if let Ok(url) = reqwest::Url::parse(&state.url) {
        let path = url.path().to_lowercase();
        state.blocked = authentication && path.contains("/signin/rejected");
        state.mfa = authentication
            && [
                "/challenge/totp",
                "/challenge/ipp",
                "/challenge/dp",
                "/challenge/az",
                "/challenge/ootp",
                "/challenge/selection",
                "/challenge/sk",
                "/challenge/pk",
            ]
            .iter()
            .any(|p| path.contains(p));
    }
    // Never scan mailbox/message text: service pages need only account controls.
    let kinds = if authentication {
        vec![
            UIA_EditControlTypeId,
            UIA_CheckBoxControlTypeId,
            UIA_ButtonControlTypeId,
            UIA_HyperlinkControlTypeId,
            UIA_TextControlTypeId,
        ]
    } else {
        vec![UIA_ButtonControlTypeId, UIA_HyperlinkControlTypeId]
    };
    for kind in kinds {
        let Some(elements) = find_targeted_elements(automation, window, kind) else {
            continue;
        };
        for index in 0..unsafe { elements.Length() }.unwrap_or(0).min(120) {
            let Ok(element) = (unsafe { elements.GetElement(index) }) else {
                continue;
            };
            if unsafe { element.CurrentIsOffscreen() }.map_or(true, |v| v.as_bool())
                || !document_field(automation, &element)
            {
                continue;
            }
            let name = unsafe { element.CurrentName() }
                .map(|v| v.to_string())
                .unwrap_or_default();
            if !authentication {
                match account_control(&name, identifier) {
                    Some(true) => state.account_matches = true,
                    Some(false) => state.other_account = true,
                    None => {}
                }
                continue;
            }
            // Google's password-step account chip is a button. Do not infer the
            // selected identity from arbitrary instructions or recovery body text.
            if kind == UIA_ButtonControlTypeId || kind == UIA_HyperlinkControlTypeId {
                match native_state::email_control(&name, identifier) {
                    Some(true) => state.account_matches = true,
                    Some(false) => state.other_account = true,
                    None => {}
                }
            }
            let lower: String = name.chars().take(400).collect::<String>().to_lowercase();
            let id = unsafe { element.CurrentAutomationId() }
                .map(|v| v.to_string().to_lowercase())
                .unwrap_or_default();
            if kind == UIA_EditControlTypeId {
                let protected = unsafe { element.CurrentIsPassword() }.is_ok_and(|v| v.as_bool());
                state.password |= protected;
                state.identifier |= id == "identifierid" && !protected;
                let aria = unsafe { element.CurrentAriaProperties() }
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                // IsDataValidForForm defaults to false when unsupported; only explicit ARIA invalid counts.
                state.invalid |= (protected || id == "identifierid")
                    && aria.split(';').any(|p| p.trim() == "invalid=true");
                state.mfa |=
                    ["totppin", "idvpin", "idvanyphonepin", "ootppin"].contains(&id.as_str());
            }
            state.captcha |= (kind == UIA_CheckBoxControlTypeId
                && ["robot", "roboter", "机器人", "ロボット"]
                    .iter()
                    .any(|p| lower.contains(p)))
                || ["recaptcha", "captchaimg", "captchainput"]
                    .iter()
                    .any(|p| id.contains(p));
            state.locked |= [
                "too many failed attempts",
                "too many attempts",
                "account has been disabled",
                "account is locked",
                "för många försök",
                "för många misslyckade",
                "kontot har inaktiverats",
            ]
            .iter()
            .any(|p| lower.contains(p));
            state.invalid |= [
                "wrong password",
                "incorrect password",
                "couldn’t find your google account",
                "couldn't find your google account",
                "fel lösenord",
                "felaktigt lösenord",
                "hittade inte ditt google",
                "falsches passwort",
                "mot de passe incorrect",
                "contraseña incorrecta",
            ]
            .iter()
            .any(|p| lower.contains(p));
            state.blocked |= [
                "browser or app may not be secure",
                "webbläsaren eller appen kanske inte är säker",
            ]
            .iter()
            .any(|p| lower.contains(p));
        }
    }
    state
}
