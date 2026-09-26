(() => {
    const expected = __EXPECTED_IDENTIFIER__;
    const result = {
        has_password_input: false,
        has_error_message: false,
        has_captcha: false,
        captcha_type: '',
        has_mfa_input: false,
        has_mfa_text: false,
        mfa_type: '',
        has_account_locked: false,
        locked_text: '',
        url: window.location.href,
        title: document.title || '',
        authenticated_control: false,
        has_identifier_input: false,
        identity_match: false,
        identity_mismatch: false,
        identity_ambiguous: false,
        ready: document.readyState,
    };

    const visible = el => el.getClientRects().length > 0 && !el.hidden &&
        (!el.checkVisibility || el.checkVisibility({checkOpacity:true,checkVisibilityCSS:true}));
    result.has_password_input = Array.from(document.querySelectorAll('input[type="password"]')).some(visible);
    result.has_identifier_input = Array.from(document.querySelectorAll('input[autocomplete~="username"], input[type="email"]')).some(visible);
    result.has_error_message = Array.from(document.querySelectorAll('input[aria-invalid="true"]')).some(visible);
    // Absence of a password field is not success: loading/error/identifier pages also lack one.
    const logoutAction = el => {
        const label = (el.getAttribute('aria-label') || el.textContent || '').trim().toLowerCase();
        // Form actions/link targets provide a language-independent signal where exposed.
        let action = '';
        try {
            const target = new URL(el.getAttribute('href') || el.getAttribute('formaction') || el.getAttribute('action') || el.closest('form')?.getAttribute('action') || '', location.href);
            if (target.origin !== location.origin) return false;
            action = target.pathname;
        } catch { return false; }
        return /(^|\/)(logout|signout|sign-out|log-out)(\/|$)/i.test(action)
            || ['sign out','log out','logout','logga ut','abmelden','se déconnecter','cerrar sesión'].includes(label);
    };
    result.authenticated_control = [...document.querySelectorAll('a[href], button, [role="button"], form[action]')].some(el => visible(el) && logoutAction(el));
    // Closed account menus still describe a signed-in UI when a visible trigger
    // explicitly owns the menu containing a same-origin logout action.
    const triggers = [...document.querySelectorAll('button[aria-controls], [aria-haspopup][aria-controls], [aria-owns], details > summary')].filter(visible);
    result.authenticated_control ||= triggers.some(trigger => {
        const targets = (trigger.getAttribute('aria-controls') || trigger.getAttribute('aria-owns') || '').split(/\s+/).filter(Boolean).map(id => document.getElementById(id)).filter(Boolean);
        if (trigger.tagName === 'SUMMARY') targets.push(trigger.parentElement);
        return targets.some(target => [...target.querySelectorAll('a[href], button, form[action]')].some(logoutAction));
    });
    // GitHub renders the authenticated viewer separately from repository owners.
    // Never use repository/author analytics metadata as the current account.
    if (location.protocol === 'https:' && location.hostname === 'github.com'
        && document.body?.classList.contains('logged-in')) {
        const login = document.querySelector('meta[name="user-login"]')?.content?.trim().toLowerCase();
        if (login && /^[a-z0-9](?:[a-z0-9-]{0,38})$/i.test(login)) {
            result.authenticated_control = true;
            if (expected && !expected.includes('@')) {
                result.identity_match = login === expected;
                result.identity_mismatch = login !== expected;
            }
        }
    }
    if (expected.includes('@')) {
        const identities = new Set();
        for (const el of document.querySelectorAll('header button, header a, [role="banner"] button, [role="navigation"] button, [aria-haspopup="menu"], [role="menuitem"]')) {
            if (!visible(el)) continue;
            const label = (el.getAttribute('aria-label') || el.getAttribute('title') || '').toLowerCase();
            const emails = label.split(/[^\p{L}\p{N}@._+\-]+/u).filter(part => part.includes('@'));
            if (emails.length === 1) identities.add(emails[0]);
        }
        result.identity_match = identities.size === 1 && identities.has(expected);
        result.identity_mismatch = identities.size > 0 && !identities.has(expected);
        result.identity_ambiguous = identities.size > 1;
    }

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
            if (!visible(el)) continue;
            const text = (el.textContent || '').trim().toLowerCase();
            if (text.length > 3 && text.length < 300) {
                for (const pattern of errorPatterns) {
                    if (text.includes(pattern)) {
                        result.has_error_message = true;
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
    const captchaCompleted = Array.from(document.querySelectorAll('[name="g-recaptcha-response"], [name="h-captcha-response"], [name="cf-turnstile-response"]')).some(el => !!el.value);
    for (const sel of captchaSelectors) {
        if (!captchaCompleted && Array.from(document.querySelectorAll(sel)).some(visible)) {
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
    if (Array.from(otpInputs).some(visible)) {
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
