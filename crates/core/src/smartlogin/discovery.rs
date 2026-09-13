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
    ("gmail.com", "https://accounts.google.com/AddSession?service=mail"),
    ("google.com", "https://accounts.google.com/AddSession"),
    ("youtube.com", "https://accounts.google.com/AddSession?service=youtube"),
    ("drive.google.com", "https://accounts.google.com/AddSession?service=wise"),
    ("outlook.com", "https://login.live.com/"),
    ("outlook.live.com", "https://login.live.com/"),
    ("live.com", "https://login.live.com/"),
    ("hotmail.com", "https://login.live.com/"),
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

    // Direct: password field on a simple page
    let has_password = snapshot.inputs.iter().any(|i| i.input_type == "password");
    if has_password && is_simple_page {
        return true;
    }

    // Check for identifier inputs (email, tel, or text with user/email hints)
    let has_email_or_tel = snapshot.inputs.iter().any(|i| {
        matches!(i.input_type.as_str(), "email" | "tel")
    });

    let has_identifier = snapshot.inputs.iter().any(|i| is_likely_identifier_input(i));

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

/// Detect if the user is already logged into a dashboard/inbox
/// (no login form, no login links, but has user-specific content).
pub fn is_likely_logged_in(snapshot: &PageSnapshot) -> bool {
    let has_password = snapshot.inputs.iter().any(|i| i.input_type == "password");
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
        let score = score_login_link(link);
        if score > 0.25 {
            candidates.push(ScoredLoginLink {
                link: link.clone(),
                confidence: score,
            });
        }
    }

    for button in &snapshot.buttons {
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

    score.max(0.0).min(1.0)
}

/// Check if a target URL is an allowed auth domain for the given entry URL.
/// Supports known auth domain pairs and same-base-domain navigation.
pub fn is_allowed_auth_domain(entry_url: &str, target_url: &str) -> bool {
    if domains_match(entry_url, target_url) {
        return true;
    }

    let entry_domain = match extract_domain(entry_url) {
        Some(d) => d,
        None => return false,
    };
    let target_domain = match extract_domain(target_url) {
        Some(d) => d,
        None => return false,
    };

    // Check known auth domain mappings
    for (service, auth) in AUTH_DOMAINS {
        if entry_domain.ends_with(service) && target_domain.ends_with(auth) {
            return true;
        }
    }

    // Allow same base domain (e.g., auth.example.com → example.com)
    let entry_base = base_domain(&entry_domain);
    let target_base = base_domain(&target_domain);
    if !entry_base.is_empty() && entry_base == target_base {
        return true;
    }

    // Allow if target URL path contains login-related segments
    let target_lower = target_url.to_lowercase();
    if LOGIN_PATH_PATTERNS.iter().any(|p| target_lower.contains(p)) {
        return true;
    }

    false
}

/// Get login probe URLs to try for a given entry URL.
/// For known services, returns the direct login URL.
/// For generic sites, tries common login paths on the same domain.
pub fn get_probe_urls(base_url: &str) -> Vec<String> {
    let normalized = normalize_url(base_url);
    let domain = extract_domain(&normalized).unwrap_or_default();

    let mut urls = Vec::new();

    // Check known service login URLs first (optimization)
    for (service, login_url) in SERVICE_LOGIN_URLS {
        if domain.ends_with(service) {
            urls.push(login_url.to_string());
            return urls;
        }
    }

    // Generic: try common login paths on the same domain
    for path in PROBE_PATHS {
        urls.push(format!("https://{domain}{path}"));
    }

    urls
}

/// Extract the domain from a URL for comparison.
pub fn extract_domain(url: &str) -> Option<String> {
    let url = url.trim();
    let without_proto = if let Some(pos) = url.find("://") {
        &url[pos + 3..]
    } else {
        url
    };
    let domain = without_proto.split('/').next().unwrap_or(without_proto);
    let domain = domain.split(':').next().unwrap_or(domain);
    let domain = domain.strip_prefix("www.").unwrap_or(domain);

    if domain.is_empty() {
        None
    } else {
        Some(domain.to_lowercase())
    }
}

/// Extract the base domain (e.g., "accounts.google.com" → "google.com").
fn base_domain(domain: &str) -> String {
    let parts: Vec<&str> = domain.split('.').collect();
    if parts.len() >= 2 {
        parts[parts.len() - 2..].join(".")
    } else {
        domain.to_string()
    }
}

/// Check if two URLs belong to the same domain (including subdomains).
pub fn domains_match(url_a: &str, url_b: &str) -> bool {
    let a = match extract_domain(url_a) {
        Some(d) => d,
        None => return false,
    };
    let b = match extract_domain(url_b) {
        Some(d) => d,
        None => return false,
    };

    if a == b {
        return true;
    }

    a.ends_with(&format!(".{b}")) || b.ends_with(&format!(".{a}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_domain() {
        assert_eq!(extract_domain("https://github.com/login"), Some("github.com".into()));
        assert_eq!(extract_domain("https://www.google.com"), Some("google.com".into()));
        assert_eq!(extract_domain("http://example.com:8080/path"), Some("example.com".into()));
    }

    #[test]
    fn test_domains_match() {
        assert!(domains_match("https://github.com", "https://github.com/login"));
        assert!(domains_match("https://accounts.google.com", "https://google.com"));
        assert!(!domains_match("https://github.com", "https://evil.com"));
    }

    #[test]
    fn test_auth_domain_allowed() {
        assert!(is_allowed_auth_domain("https://gmail.com", "https://accounts.google.com/signin"));
        assert!(is_allowed_auth_domain("https://outlook.com", "https://login.microsoftonline.com"));
        assert!(!is_allowed_auth_domain("https://gmail.com", "https://evil.com"));
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
    fn test_probe_urls_gmail() {
        let probes = get_probe_urls("gmail.com");
        assert!(probes[0].contains("accounts.google.com"));
    }

    #[test]
    fn test_probe_urls_generic() {
        let probes = get_probe_urls("example.com");
        assert!(probes.iter().any(|p| p.contains("/login")));
    }
}
