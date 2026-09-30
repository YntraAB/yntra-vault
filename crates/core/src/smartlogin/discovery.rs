//! Login Discovery — determines whether a login form exists on the current page,
//! and if not, finds and navigates to candidate login links/buttons.
//! Handles cross-domain auth flows (e.g., gmail.com → accounts.google.com)
//! and common login path probing.

use crate::smartlogin::types::*;
use crate::smartlogin::logging::{SmartLoginLogger, SmartLoginEvent, SmartLoginEventDetail};

/// Multilingual patterns that indicate a login-related link or button.
const LOGIN_LINK_PATTERNS: &[&str] = &[
    "sign in", "signin", "log in", "login", "logga in", "anmelden",
    "iniciar sesión", "se connecter", "connexion", "inloggen",
    "entrar", "accedi", "zaloguj", "přihlásit", "войти",
    "ログイン", "登录", "登入", "로그인",
    "my account", "mitt konto", "mein konto", "mon compte",
    "add account", "lägg till konto", "konto hinzufügen",
    "another account", "annat konto", "use another",
    "switch account", "byt konto",
];

/// URL path segments that suggest a login page.
const LOGIN_PATH_PATTERNS: &[&str] = &[
    "/login", "/signin", "/sign-in", "/sign_in", "/log-in",
    "/auth", "/authenticate", "/session", "/sso",
    "/account/login", "/accounts/login", "/user/login",
    "/logga-in", "/anmelden", "/connexion",
    "/ServiceLogin", "/v2/identifier",
];

/// Patterns that suggest "sign up" (excluded).
const SIGNUP_PATTERNS: &[&str] = &[
    "sign up", "signup", "register", "create account", "join",
    "registrera", "registrieren", "s'inscrire", "registrarse",
    "get started", "free trial", "skapa konto",
];

/// Patterns that indicate non-login navigation (strong negatives).
const EXCLUDED_PATTERNS: &[&str] = &[
    "help", "support", "contact", "about", "privacy", "terms",
    "cookie", "policy", "faq", "hjälp", "kontakt", "om oss",
    "hilfe", "aide", "ayuda", "feedback", "report", "learn more",
    "download", "install", "blog", "news", "press", "careers",
    "developer", "api", "status", "security", "legal",
    "forgot", "reset password", "glömt",
    "manage", "hantera", "verwalten", "gérer", "administrar",
];

/// Common login paths to probe when no login form or links are found.
const PROBE_PATHS: &[&str] = &[
    "/login", "/signin", "/sign-in", "/auth/login",
    "/account/login", "/accounts/login",
];

/// Known service → full login URL mappings.
/// These bypass the main page (which may show a dashboard when logged in)
/// and go directly to the login/add-account flow.
const SERVICE_LOGIN_URLS: &[(&str, &str)] = &[
    ("github.com", "https://github.com/login"),
    ("gmail.com", "https://accounts.google.com/AddSession?service=mail"),
    ("google.com", "https://accounts.google.com/AddSession"),
    ("youtube.com", "https://accounts.google.com/AddSession?service=youtube"),
    ("drive.google.com", "https://accounts.google.com/AddSession?service=wise"),
    ("outlook.com", "https://login.live.com/"),
    ("outlook.live.com", "https://login.live.com/"),
    ("live.com", "https://login.live.com/"),
    ("hotmail.com", "https://login.live.com/"),
    ("steampowered.com", "https://store.steampowered.com/login/"),
    ("steamcommunity.com", "https://steamcommunity.com/login/home/"),
];

/// Auth domains that are allowed for cross-domain navigation.
const AUTH_DOMAINS: &[(&str, &str)] = &[
    ("gmail.com", "accounts.google.com"),
    ("google.com", "accounts.google.com"),
    ("youtube.com", "accounts.google.com"),
    ("drive.google.com", "accounts.google.com"),
    ("outlook.com", "login.microsoftonline.com"),
    ("outlook.com", "login.live.com"),
    ("outlook.live.com", "login.live.com"),
    ("live.com", "login.live.com"),
    ("hotmail.com", "login.live.com"),
    ("github.com", "github.com"),
    ("facebook.com", "www.facebook.com"),
    ("instagram.com", "www.instagram.com"),
    ("twitter.com", "twitter.com"),
    ("x.com", "twitter.com"),
    ("linkedin.com", "www.linkedin.com"),
    ("amazon.com", "www.amazon.com"),
    ("reddit.com", "www.reddit.com"),
    ("steampowered.com", "steamcommunity.com"),
    ("store.steampowered.com", "steamcommunity.com"),
    ("steampowered.com", "store.steampowered.com"),
    ("steamcommunity.com", "store.steampowered.com"),
    ("steamcommunity.com", "steampowered.com"),
    // Proton Mail starts on the product host and redirects its credential
    // form to the dedicated account host. Keep this explicit so unrelated
    // Proton subdomains remain outside the trust boundary.
    ("proton.me", "account.proton.me"),
    ("mail.proton.me", "account.proton.me"),
];

/// Normalize a user-provided URL for navigation.
/// Ensures https:// prefix and strips trailing whitespace.
pub fn normalize_url(url: &str) -> String {
    let url = url.trim();
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        format!("https://{url}")
    }
}

/// Check if a login form is already present on the page.
pub fn has_login_form(snapshot: &PageSnapshot) -> bool {
    // Complex pages (many buttons/links) are NOT standalone login forms.
    // Force navigation to /login instead of filling in-place.
    let is_simple_page = snapshot.buttons.len() <= 10 && snapshot.links.len() <= 20;

    // Some providers render an authenticated login chooser before they create
    // any input. Treat a clearly labelled chooser on a login URL
    // as the login form so discovery does not wander through unrelated paths.
    if has_login_method_chooser(snapshot) {
        return true;
    }

    // Direct: password field on a simple page
    let has_password = snapshot.inputs.iter().any(|i| i.is_visible && !i.is_readonly && i.input_type == "password");
    if has_password && is_simple_page {
        return true;
    }

    // Check for identifier inputs (email, tel, or text with user/email hints)
    let has_email_or_tel = snapshot.inputs.iter().any(|i| {
        i.is_visible && !i.is_readonly && matches!(i.input_type.as_str(), "email" | "tel")
    });

    let has_identifier = snapshot.inputs.iter().any(is_likely_identifier_input);

    // Check for Next/Continue/Sign-in buttons (multilingual)
    let has_action_button = snapshot.buttons.iter().any(|b| {
        let text = b.text.to_lowercase();
        let label = b.aria_label.to_lowercase();
        let combined = format!("{text} {label}");
        LOGIN_LINK_PATTERNS.iter().any(|p| combined.contains(p))
            || combined.contains("continue")
            || combined.contains("next")
            || combined.contains("submit")
            || combined.contains("go")
            // Swedish
            || combined.contains("nästa")
            || combined.contains("fortsätt")
            || combined.contains("logga in")
            // German
            || combined.contains("weiter")
            || combined.contains("anmelden")
            // French
            || combined.contains("continuer")
            || combined.contains("suivant")
            // Spanish
            || combined.contains("siguiente")
            || combined.contains("continuar")
            // Italian
            || combined.contains("avanti")
            || combined.contains("accedi")
            // Portuguese
            || combined.contains("entrar")
            || combined.contains("próximo")
            // Dutch
            || combined.contains("volgende")
            // Polish
            || combined.contains("dalej")
            // Russian
            || combined.contains("далее")
            || combined.contains("войти")
    });
    // All heuristic checks below require a simple page — reject complex pages
    if !is_simple_page {
        return false;
    }

    // Email/tel input + action button = login form
    if has_email_or_tel && has_action_button {
        return true;
    }

    // Identifier input with auth context + action button
    let has_auth_input = snapshot.inputs.iter().any(|i| {
        is_likely_identifier_input(i) && has_auth_context(i)
    });
    if has_auth_input && has_action_button {
        return true;
    }

    // Minimal page heuristic: few inputs (1-3) + identifier + action button
    if has_identifier && has_action_button && snapshot.inputs.len() <= 3 {
        return true;
    }

    // Very minimal page: 1-2 inputs + identifier + any button at all
    if has_identifier && !snapshot.buttons.is_empty() && snapshot.inputs.len() <= 2 {
        return true;
    }

    false
}

/// Returns true for a login/auth page whose visible controls select an
/// authentication method instead of exposing an identifier input immediately.
/// This is deliberately stricter than matching a lone "Continue" button: the
/// page must be on a login-like path and expose a known method label.
pub fn has_login_method_chooser(snapshot: &PageSnapshot) -> bool {
    if !is_login_url(&snapshot.url) {
        return false;
    }

    let method_labels = [
        "phone", "mobile", "telephone", "teléfono", "téléphone", "telefon", "телефон", "email", "e-mail", "correo", "courriel", "e-post", "邮箱", "username",
        "user name", "usuario", "utilisateur", "användarnamn", "benutzername", "имя пользователя", "用户名", "qr code", "passkey", "google", "facebook", "apple",
        "microsoft", "use another account", "annat konto", "anderes konto", "otro método",
    ];
    let is_method_control = |text: &str, aria_label: &str| {
        let combined = format!("{text} {aria_label}").to_ascii_lowercase();
        method_labels.iter().any(|pattern| combined.contains(pattern))
    };
    let method_controls = snapshot.buttons.iter().filter(|button| {
        button.is_visible && is_method_control(&button.text, &button.aria_label)
    }).count() + snapshot.links.iter().filter(|link| {
        link.is_visible && is_method_control(&link.text, &link.aria_label)
    }).count();

    // One explicit method selector is sufficient. Social/QR-only choosers are
    // also valid because they are still a user-visible authentication choice.
    method_controls > 0
}

/// Returns true when a URL is already on a conventional authentication path.
/// Discovery must not leave such a page just because its controls are rendered
/// unusually or its challenge is not yet understood.
pub fn is_login_url(value: &str) -> bool {
    let Ok(url) = url::Url::parse(value) else { return false; };
    let path = url.path().to_ascii_lowercase();
    LOGIN_PATH_PATTERNS.iter().any(|pattern| {
        let pattern = pattern.to_ascii_lowercase();
        path == pattern || path.starts_with(&format!("{pattern}/"))
    })
}

/// Detect if the user is already logged into a dashboard/inbox
/// (no login form, no login links, but has user-specific content).
pub fn is_likely_logged_in(snapshot: &PageSnapshot) -> bool {
    let has_password = snapshot.inputs.iter().any(|i| i.is_visible && !i.is_readonly && i.input_type == "password");
    if has_password {
        return false;
    }

    // Many buttons/links typically indicates an app UI, not a login page
    let has_complex_ui = snapshot.buttons.len() > 15 || snapshot.links.len() > 10;

    // No login-related elements
    let has_login_elements = snapshot.buttons.iter().any(|b| {
        let text = b.text.to_lowercase();
        LOGIN_LINK_PATTERNS.iter().any(|p| text.contains(p))
    }) || snapshot.links.iter().any(|l| {
        let text = l.text.to_lowercase();
        LOGIN_LINK_PATTERNS.iter().any(|p| text.contains(p))
    });

    has_complex_ui && !has_login_elements
}

fn is_likely_identifier_input(input: &InputInfo) -> bool {
    if !input.is_visible || input.is_readonly { return false; }
    let t = input.input_type.as_str();
    if matches!(t, "email" | "tel") {
        return true;
    }
    if t != "text" && t != "search" {
        return false;
    }
    let all_text = format!(
        "{} {} {} {} {} {}",
        input.name, input.id, input.placeholder, input.autocomplete,
        input.aria_label, input.associated_label,
    )
    .to_lowercase();

    ["user", "email", "login", "phone", "tel", "account", "identifier"]
        .iter()
        .any(|kw| all_text.contains(kw))
}

fn has_auth_context(input: &InputInfo) -> bool {
    let context = format!(
        "{} {} {}",
        input.surrounding_text, input.associated_label, input.aria_label,
    )
    .to_lowercase();

    LOGIN_LINK_PATTERNS.iter().any(|p| context.contains(p))
        || context.contains("password")
        || context.contains("username")
}

/// Score login-related links on the page.
pub fn find_login_links(
    snapshot: &PageSnapshot,
    logger: &SmartLoginLogger,
) -> Vec<ScoredLoginLink> {
    let mut candidates: Vec<ScoredLoginLink> = Vec::new();

    for link in &snapshot.links {
        if !link.is_visible || (!link.href.is_empty() && !login_target_allowed(&snapshot.url, &link.href)) { continue; }
        let score = score_login_link(link);
        if score > 0.25 {
            candidates.push(ScoredLoginLink {
                link: link.clone(),
                confidence: score,
            });
        }
    }

    for button in &snapshot.buttons {
        if !button.is_visible { continue; }
        let score = score_login_button_as_nav(button);
        if score > 0.25 {
            candidates.push(ScoredLoginLink {
                link: LinkInfo {
                    backend_node_id: button.backend_node_id,
                    text: button.text.clone(),
                    href: String::new(),
                    aria_label: button.aria_label.clone(),
                    is_visible: button.is_visible,
                    in_nav: false,
                    ax_role: button.ax_role.clone(),
                    ax_name: button.ax_name.clone(),
                },
                confidence: score,
            });
        }
    }

    candidates.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

    for (i, c) in candidates.iter().take(3).enumerate() {
        logger.emit(
            SmartLoginEvent::new(
                LoginState::SearchingForLogin,
                format!(
                    "Login candidate #{}: \"{}\" (confidence: {:.2})",
                    i + 1,
                    c.link.text,
                    c.confidence,
                ),
            )
            .with_detail(SmartLoginEventDetail::LoginLinkFound {
                text: c.link.text.clone(),
                href: c.link.href.clone(),
                confidence: c.confidence,
            }),
        );
    }

    candidates
}

#[derive(Debug, Clone)]
pub struct ScoredLoginLink {
    pub link: LinkInfo,
    pub confidence: f64,
}

fn score_login_link(link: &LinkInfo) -> f64 {
    let mut score: f64 = 0.0;
    let text = link.text.to_lowercase().trim().to_string();
    let href = link.href.to_lowercase();
    let aria = link.aria_label.to_lowercase();
    let combined = format!("{text} {aria}");

    // Hard exclude: sign-up
    if SIGNUP_PATTERNS.iter().any(|p| combined.contains(p)) {
        return 0.0;
    }

    // Hard exclude: non-login navigation
    if EXCLUDED_PATTERNS.iter().any(|p| text.contains(p)) {
        return 0.0;
    }

    // Empty text links are usually icons/images — low confidence
    if text.is_empty() {
        return 0.0;
    }

    // Text match against login patterns
    for (i, pattern) in LOGIN_LINK_PATTERNS.iter().enumerate() {
        if combined.contains(pattern) {
            let weight = 0.50 - (i as f64 * 0.01).min(0.20);
            score += weight;
            break;
        }
    }

    // URL path match
    for pattern in LOGIN_PATH_PATTERNS {
        if href.contains(pattern) {
            score += 0.30;
            break;
        }
    }

    // Bonus: in navigation/header
    if link.in_nav {
        score += 0.10;
    }

    // Bonus: ARIA label
    if LOGIN_LINK_PATTERNS.iter().any(|p| aria.contains(p)) {
        score += 0.10;
    }

    score.min(1.0)
}

fn score_login_button_as_nav(button: &ButtonInfo) -> f64 {
    let mut score: f64 = 0.0;
    let text = button.text.to_lowercase().trim().to_string();
    let aria = button.aria_label.to_lowercase();
    let combined = format!("{text} {aria}");

    if SIGNUP_PATTERNS.iter().any(|p| combined.contains(p)) {
        return 0.0;
    }

    if EXCLUDED_PATTERNS.iter().any(|p| text.contains(p)) {
        return 0.0;
    }

    if text.is_empty() {
        return 0.0;
    }

    for pattern in LOGIN_LINK_PATTERNS {
        if combined.contains(pattern) {
            score += 0.45;
            break;
        }
    }

    if LOGIN_LINK_PATTERNS.iter().any(|p| aria.contains(p)) {
        score += 0.10;
    }

    if button.form_index.is_some() {
        score -= 0.15;
    }

    score.clamp(0.0, 1.0)
}

/// Parse the credential security boundary once, rejecting ambiguous authorities.
/// Exact origins avoid public/private-suffix tenant confusion without heuristics.
pub fn credential_url(value: &str) -> Option<url::Url> {
    if value.contains('\\') || value.chars().any(|c| c.is_control()) { return None; }
    let parsed = url::Url::parse(&normalize_url(value)).ok()?;
    let local_http = parsed.scheme() == "http" && matches!(parsed.host_str(),Some("localhost"|"127.0.0.1"|"[::1]"));
    if (parsed.scheme() != "https" && !local_http) || parsed.host_str().is_none()
        || !parsed.username().is_empty() || parsed.password().is_some() { return None; }
    Some(parsed)
}

pub fn is_allowed_auth_domain(entry_url: &str, target_url: &str) -> bool {
    let (Some(entry), Some(target)) = (credential_url(entry_url),credential_url(target_url)) else { return false; };
    if entry.origin() == target.origin() { return true; }
    // SSO exceptions are exact hostname pairs on the standard HTTPS port only.
    if entry.port_or_known_default() != Some(443) || target.port_or_known_default() != Some(443) { return false; }
    AUTH_DOMAINS.iter().any(|(service,auth)| entry.host_str() == Some(*service) && target.host_str() == Some(*auth))
}
/// Get login probe URLs to try for a given entry URL.
/// For known services, returns the direct login URL.
/// For generic sites, tries common login paths on the same domain.
pub fn get_probe_urls(base_url: &str) -> Vec<String> {
    let normalized = normalize_url(base_url);
    let Some(parsed) = credential_url(&normalized) else { return Vec::new(); };
    let domain = parsed.host_str().unwrap_or_default();
    let authority = match parsed.port() {
        Some(port) => format!("{domain}:{port}"),
        None => domain.to_string(),
    };

    let mut urls = Vec::new();

    // Check known service login URLs first (optimization)
    for (service, login_url) in SERVICE_LOGIN_URLS {
        let domain_matches = domain == *service || domain.ends_with(&format!(".{}", service));
        if domain_matches && parsed.port_or_known_default() == Some(443) {
            urls.push(login_url.to_string());
            return urls;
        }
    }

    // Generic: try common login paths on the same domain
    for path in PROBE_PATHS {
        urls.push(format!("{}://{authority}{path}", parsed.scheme()));
    }

    urls
}

/// Content platforms own arbitrary user/repository paths. Do not infer their
/// authentication routes from a repository name or its "login" link text.
pub fn login_target_allowed(entry_url: &str, target_url: &str) -> bool {
    if !is_allowed_auth_domain(entry_url, target_url) { return false; }
    let github = reqwest::Url::parse(&normalize_url(entry_url)).is_ok_and(|u| matches!(u.host_str(), Some("github.com" | "www.github.com")));
    if !github { return true; }
    reqwest::Url::parse(target_url).is_ok_and(|u| {
        u.scheme() == "https" && u.host_str() == Some("github.com")
            && u.username().is_empty() && u.password().is_none()
            && (matches!(u.path(), "/login" | "/session") || u.path().starts_with("/login/") || u.path().starts_with("/sessions/"))
    })
}

/// URL-parser host extraction; this function does not establish credential trust.
pub fn extract_domain(value: &str) -> Option<String> {
    credential_url(value)?.host_str().map(str::to_owned)
}

pub fn domains_match(url_a: &str, url_b: &str) -> bool {
    match (credential_url(url_a),credential_url(url_b)) {
        (Some(a),Some(b)) => a.origin() == b.origin(),
        _ => false,
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_discovery_never_probes_user_or_repository_paths() {
        assert_eq!(get_probe_urls("https://github.com/demo/project"), vec!["https://github.com/login"]);
        assert!(login_target_allowed("https://github.com", "https://github.com/login?return_to=%2F"));
        for target in ["https://github.com/signin", "https://github.com/auth/login", "https://github.com/demo/login", "https://gist.github.com/login", "https://github.com.evil.test/login", "http://github.com/login", "https://user@github.com/login"] {
            assert!(!login_target_allowed("https://github.com", target), "{target}");
        }
    }

    #[test]
    fn test_extract_domain() {
        assert_eq!(extract_domain("https://github.com/login"), Some("github.com".into()));
        assert_eq!(extract_domain("https://www.google.com"), Some("www.google.com".into()));
        assert_eq!(extract_domain("http://example.com:8080/path"), None);
    }

    #[test]
    fn test_domains_match() {
        assert!(domains_match("https://github.com", "https://github.com/login"));
        assert!(!domains_match("https://accounts.google.com", "https://google.com"));
        assert!(!domains_match("https://github.com", "https://evil.com"));
    }

    #[test]
    fn test_auth_domain_allowed() {
        assert!(is_allowed_auth_domain("https://gmail.com", "https://accounts.google.com/signin"));
        assert!(is_allowed_auth_domain("https://outlook.com", "https://login.microsoftonline.com"));
        assert!(is_allowed_auth_domain("https://store.steampowered.com", "https://steamcommunity.com/login/home/"));
        assert!(is_allowed_auth_domain("https://steamcommunity.com", "https://store.steampowered.com/login/"));
        assert!(!is_allowed_auth_domain("https://gmail.com", "https://evil.com"));
        assert!(!is_allowed_auth_domain("https://gmail.com", "https://evil.com/login"));
        assert!(!is_allowed_auth_domain("https://bank.com", "https://attacker.com/signin"));
        assert!(!is_allowed_auth_domain("https://mysite.com", "https://phishing.com/auth/login"));
    }

    #[test]
    fn test_excluded_patterns() {
        let link = LinkInfo {
            backend_node_id: 0,
            text: "Help".into(),
            href: "https://support.google.com".into(),
            aria_label: String::new(),
            is_visible: true,
            in_nav: false,
            ax_role: "link".into(),
            ax_name: String::new(),
        };
        assert_eq!(score_login_link(&link), 0.0);
    }

    #[test]
    fn test_signup_link_excluded() {
        let link = LinkInfo {
            backend_node_id: 0,
            text: "Sign up for free".into(),
            href: "/signup".into(),
            aria_label: String::new(),
            is_visible: true,
            in_nav: true,
            ax_role: "link".into(),
            ax_name: String::new(),
        };
        assert_eq!(score_login_link(&link), 0.0);
    }

    #[test]
    fn test_signin_link_scored_high() {
        let link = LinkInfo {
            backend_node_id: 0,
            text: "Sign in".into(),
            href: "/login".into(),
            aria_label: String::new(),
            is_visible: true,
            in_nav: true,
            ax_role: "link".into(),
            ax_name: String::new(),
        };
        let score = score_login_link(&link);
        assert!(score > 0.7, "Expected high score for 'Sign in' + '/login', got {score}");
    }

    #[test]
    fn tiktok_method_chooser_stops_discovery_on_login_page() {
        let snapshot = PageSnapshot {
            url: "https://www.tiktok.com/login".into(),
            title: "Log in to TikTok".into(),
            forms: vec![],
            inputs: vec![],
            buttons: vec![
                ButtonInfo {
                    backend_node_id: 0,
                    text: "Use QR code".into(),
                    button_type: String::new(),
                    aria_label: String::new(),
                    is_visible: true,
                    form_index: None,
                    ax_role: "button".into(),
                    ax_name: String::new(),
                },
                ButtonInfo {
                    backend_node_id: 0,
                    text: "Use phone / email / username".into(),
                    button_type: String::new(),
                    aria_label: String::new(),
                    is_visible: true,
                    form_index: None,
                    ax_role: "button".into(),
                    ax_name: String::new(),
                },
            ],
            links: vec![],
        };
        assert!(has_login_method_chooser(&snapshot));
        assert!(is_login_url(&snapshot.url));
        assert!(has_login_form(&snapshot));
    }

    #[test]
    fn method_chooser_requires_login_path_and_visible_controls() {
        let mut snapshot = PageSnapshot {
            url: "https://www.tiktok.com/".into(),
            title: "TikTok".into(),
            forms: vec![],
            inputs: vec![],
            buttons: vec![ButtonInfo {
                backend_node_id: 0,
                text: "Continue with Google".into(),
                button_type: String::new(),
                aria_label: String::new(),
                is_visible: true,
                form_index: None,
                ax_role: "button".into(),
                ax_name: String::new(),
            }],
            links: vec![],
        };
        assert!(!has_login_method_chooser(&snapshot));
        assert!(!is_login_url(&snapshot.url));
        snapshot.url = "https://www.tiktok.com/login".into();
        snapshot.buttons[0].is_visible = false;
        assert!(!has_login_method_chooser(&snapshot));
    }

    #[test]
    fn test_probe_urls_gmail() {
        let probes = get_probe_urls("gmail.com");
        assert!(probes[0].contains("accounts.google.com"));
    }

    #[test]
    fn test_probe_urls_steam() {
        let probes = get_probe_urls("https://store.steampowered.com/");
        assert_eq!(probes, vec!["https://store.steampowered.com/login/"]);
    }

    #[test]
    fn test_probe_urls_generic() {
        let probes = get_probe_urls("example.com");
        assert!(probes.iter().any(|p| p.contains("/login")));
    }

    #[test]
    fn probe_urls_preserve_custom_https_ports_and_do_not_cross_domains() {
        let probes = get_probe_urls("https://example.test:8443/account");
        assert!(probes.iter().all(|probe| probe.starts_with("https://example.test:8443/")));
        assert!(probes.iter().all(|probe| login_target_allowed("https://example.test:8443/account", probe)));
        assert!(get_probe_urls("not a url").is_empty());
    }

    #[test]
    fn credential_origins_reject_cross_tenant_downgrades_userinfo_and_ports() {
        for (entry,target) in [
            ("https://victim.pages.dev","https://attacker.pages.dev/login"),
            ("https://victim.github.io","https://attacker.github.io/login"),
            ("https://bank.co.uk","https://attacker.co.uk"),
            ("https://bank.co.uk","https://auth.bank.co.uk"),
            ("https://bank.example","http://bank.example"),
            ("https://bank.example","https://bank.example:password@attacker.example"),
            ("https://bank.example","https://bank.example:8443"),
            ("https://gmail.com","https://evil.accounts.google.com"),
            ("https://tenant.google.com","https://accounts.google.com"),
        ] { assert!(!is_allowed_auth_domain(entry,target),"{entry} -> {target}"); }
        assert!(is_allowed_auth_domain("https://bank.example","https://bank.example/login"));
        assert!(is_allowed_auth_domain("https://gmail.com","https://accounts.google.com/login"));
        assert!(is_allowed_auth_domain("https://mail.proton.me/","https://account.proton.me/mail"));
        assert!(is_allowed_auth_domain("https://proton.me/","https://account.proton.me/mail"));
        assert!(!is_allowed_auth_domain("https://mail.proton.me/","https://evil.account.proton.me/mail"));
    }
}

