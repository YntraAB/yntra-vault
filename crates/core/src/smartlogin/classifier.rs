//! Field Classifier — scores each input and button using multi-signal heuristics
//! to determine their role in a login form (username, email, phone, password, submit, etc.).

use crate::smartlogin::types::*;
use crate::smartlogin::logging::{SmartLoginLogger, SmartLoginEvent, SmartLoginEventDetail};

// ─── Input Classification ───────────────────────────────────────────────

/// Multilingual patterns for identifying username fields.
const USERNAME_PATTERNS: &[&str] = &[
    "username", "user_name", "user-name", "userid", "user_id", "user-id",
    "loginname", "login_name", "login-name", "login_id",
    "identifier", "identifierid", "credential", "accountname", "account_name",
    "användarnamn", "benutzername", "nombre_usuario", "nom_utilisateur",
    "nome_utente", "usuario",
];

/// Multilingual patterns for identifying email fields.
const EMAIL_PATTERNS: &[&str] = &[
    "email", "e-mail", "mail", "emailaddress", "email_address", "email-address",
    "e-post", "epost", "correo", "courriel",
];

/// Multilingual patterns for identifying phone fields.
const PHONE_PATTERNS: &[&str] = &[
    "phone", "telephone", "tel", "mobile", "cell", "phonenumber",
    "phone_number", "phone-number", "telefon", "teléfono", "téléphone",
    "mobilnummer",
];

/// Patterns for identifying password fields.
const PASSWORD_PATTERNS: &[&str] = &[
    "password", "passwd", "pass_word", "pass-word", "passwort",
    "contraseña", "mot_de_passe", "senha", "lösenord", "hasło",
    "wachtwoord", "пароль",
];

/// Patterns for login/submit buttons.
const LOGIN_BUTTON_PATTERNS: &[&str] = &[
    "sign in", "signin", "log in", "login", "logga in", "anmelden",
    "iniciar sesión", "se connecter", "connexion", "inloggen",
    "entrar", "accedi", "zaloguj", "войти",
    "submit", "go", "enter",
    "ログイン", "登录", "登入", "로그인",
];

/// Patterns for continue/next buttons in multi-step flows.
const CONTINUE_BUTTON_PATTERNS: &[&str] = &[
    "continue", "next", "proceed", "forward",
    "fortsätt", "weiter", "continuar", "continuer", "avanti",
    "далее",
];

/// Patterns for login method selector buttons.
const METHOD_SELECTOR_PATTERNS: &[&str] = &[
    "use email", "use phone", "use username", "use password",
    "sign in with", "log in with",
    "email login", "phone login",
    "använd e-post", "använd telefon",
];

/// Classify all inputs and buttons in a page snapshot.
pub fn classify_form(
    snapshot: &PageSnapshot,
    logger: &SmartLoginLogger,
) -> ClassifiedForm {
    logger.log(LoginState::FormDetected, "Classifying form fields...");

    // Score all inputs
    let mut scored_inputs: Vec<ScoredField> = snapshot
        .inputs
        .iter()
        .map(|input| score_input(input))
        .filter(|sf| sf.confidence > 0.10)
        .collect();

    // Sort by confidence descending
    scored_inputs.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

    // Find best identifier (username/email/phone)
    let identifier_field = scored_inputs
        .iter()
        .find(|sf| matches!(sf.role, FieldRole::Username | FieldRole::Email | FieldRole::Phone))
        .cloned();

    // Find best password field
    let password_field = scored_inputs
        .iter()
        .find(|sf| sf.role == FieldRole::Password)
        .cloned();

    // Determine identifier type from the best identifier field
    let identifier_type = match &identifier_field {
        Some(f) => match f.role {
            FieldRole::Username => infer_identifier_type(&f.input),
            FieldRole::Email => IdentifierType::Email,
            FieldRole::Phone => IdentifierType::Phone,
            _ => IdentifierType::Unknown,
        },
        None => IdentifierType::Unknown,
    };

    // Score buttons
    let scored_buttons: Vec<ScoredButton> = snapshot
        .buttons
        .iter()
        .map(|b| score_button(b))
        .filter(|sb| sb.confidence > 0.10)
        .collect();

    let submit_button = scored_buttons
        .iter()
        .find(|sb| sb.role == ButtonRole::Login)
        .cloned();

    let continue_button = scored_buttons
        .iter()
        .find(|sb| sb.role == ButtonRole::Continue)
        .cloned();

    let method_selectors: Vec<ScoredButton> = scored_buttons
        .iter()
        .filter(|sb| sb.role == ButtonRole::MethodSelector)
        .cloned()
        .collect();

    // Log field classifications
    if let Some(ref f) = identifier_field {
        log_field_score(logger, f);
    }
    if let Some(ref f) = password_field {
        log_field_score(logger, f);
    }
    if let Some(ref b) = submit_button {
        log_button_score(logger, b);
    }
    if let Some(ref b) = continue_button {
        log_button_score(logger, b);
    }

    ClassifiedForm {
        identifier_field,
        password_field,
        submit_button,
        continue_button,
        method_selectors,
        identifier_type,
    }
}

/// Score a single input field across all signals.
fn score_input(input: &InputInfo) -> ScoredField {
    let mut breakdown: Vec<(String, f64)> = Vec::new();
    let mut total: f64 = 0.0;

    // Signal 1: autocomplete attribute (weight: 0.30)
    let ac = input.autocomplete.to_lowercase();
    let ac_score = match ac.as_str() {
        "username" => {
            breakdown.push(("autocomplete=username".into(), 0.30));
            0.30
        }
        "email" => {
            breakdown.push(("autocomplete=email".into(), 0.30));
            0.30
        }
        "tel" | "telephone" => {
            breakdown.push(("autocomplete=tel".into(), 0.30));
            0.30
        }
        "current-password" | "password" => {
            breakdown.push(("autocomplete=current-password".into(), 0.30));
            0.30
        }
        "new-password" => {
            // New password suggests registration, not login
            breakdown.push(("autocomplete=new-password (reduced)".into(), 0.10));
            0.10
        }
        _ => 0.0,
    };
    total += ac_score;

    // Signal 2: input type (weight: 0.20)
    let type_score = match input.input_type.as_str() {
        "password" => {
            breakdown.push(("type=password".into(), 0.20));
            0.20
        }
        "email" => {
            breakdown.push(("type=email".into(), 0.15));
            0.15
        }
        "tel" => {
            breakdown.push(("type=tel".into(), 0.15));
            0.15
        }
        "text" => {
            breakdown.push(("type=text".into(), 0.02));
            0.02
        }
        _ => 0.0,
    };
    total += type_score;

    // Signal 3: name/id pattern matching (weight: 0.15)
    let name_id = format!("{} {}", input.name, input.id).to_lowercase();
    let name_score = if PASSWORD_PATTERNS.iter().any(|p| name_id.contains(p)) {
        breakdown.push(("name/id matches password pattern".into(), 0.15));
        0.15
    } else if USERNAME_PATTERNS.iter().any(|p| name_id.contains(p)) {
        breakdown.push(("name/id matches username pattern".into(), 0.15));
        0.15
    } else if EMAIL_PATTERNS.iter().any(|p| name_id.contains(p)) {
        breakdown.push(("name/id matches email pattern".into(), 0.15));
        0.15
    } else if PHONE_PATTERNS.iter().any(|p| name_id.contains(p)) {
        breakdown.push(("name/id matches phone pattern".into(), 0.15));
        0.15
    } else {
        0.0
    };
    total += name_score;

    // Signal 4: placeholder text (weight: 0.10)
    let placeholder = input.placeholder.to_lowercase();
    let ph_score = if PASSWORD_PATTERNS.iter().any(|p| placeholder.contains(p)) {
        breakdown.push(("placeholder matches password".into(), 0.10));
        0.10
    } else if USERNAME_PATTERNS.iter().any(|p| placeholder.contains(p))
        || EMAIL_PATTERNS.iter().any(|p| placeholder.contains(p))
    {
        breakdown.push(("placeholder matches identifier".into(), 0.10));
        0.10
    } else if PHONE_PATTERNS.iter().any(|p| placeholder.contains(p)) {
        breakdown.push(("placeholder matches phone".into(), 0.10));
        0.10
    } else {
        0.0
    };
    total += ph_score;

    // Signal 5: associated label / aria-label (weight: 0.10)
    let label = format!("{} {}", input.associated_label, input.aria_label).to_lowercase();
    let label_score = if PASSWORD_PATTERNS.iter().any(|p| label.contains(p)) {
        breakdown.push(("label matches password".into(), 0.10));
        0.10
    } else if USERNAME_PATTERNS.iter().any(|p| label.contains(p))
        || EMAIL_PATTERNS.iter().any(|p| label.contains(p))
    {
        breakdown.push(("label matches identifier".into(), 0.10));
        0.10
    } else if PHONE_PATTERNS.iter().any(|p| label.contains(p)) {
        breakdown.push(("label matches phone".into(), 0.10));
        0.10
    } else {
        0.0
    };
    total += label_score;

    // Signal 6: accessibility tree name (weight: 0.08)
    let ax = input.ax_name.to_lowercase();
    let ax_score = if !ax.is_empty() {
        if PASSWORD_PATTERNS.iter().any(|p| ax.contains(p)) {
            breakdown.push(("ax_name matches password".into(), 0.08));
            0.08
        } else if USERNAME_PATTERNS.iter().any(|p| ax.contains(p))
            || EMAIL_PATTERNS.iter().any(|p| ax.contains(p))
        {
            breakdown.push(("ax_name matches identifier".into(), 0.08));
            0.08
        } else {
            0.0
        }
    } else {
        0.0
    };
    total += ax_score;

    // Signal 7: surrounding text (weight: 0.05)
    let surr = input.surrounding_text.to_lowercase();
    let surr_score = if PASSWORD_PATTERNS.iter().any(|p| surr.contains(p)) {
        breakdown.push(("surrounding text matches password".into(), 0.05));
        0.05
    } else if USERNAME_PATTERNS.iter().any(|p| surr.contains(p))
        || EMAIL_PATTERNS.iter().any(|p| surr.contains(p))
    {
        breakdown.push(("surrounding text matches identifier".into(), 0.05));
        0.05
    } else {
        0.0
    };
    total += surr_score;

    // Determine role from the strongest signals
    let role = determine_input_role(input, &breakdown);

    ScoredField {
        input: input.clone(),
        confidence: total.min(1.0),
        role,
        score_breakdown: breakdown,
    }
}

/// Determine the FieldRole from accumulated signals.
fn determine_input_role(input: &InputInfo, breakdown: &[(String, f64)]) -> FieldRole {
    // Password is easy: type=password or autocomplete=password
    if input.input_type == "password" {
        return FieldRole::Password;
    }
    if input.autocomplete.contains("password") {
        return FieldRole::Password;
    }

    let all_text = format!(
        "{} {} {} {} {} {} {}",
        input.name, input.id, input.placeholder, input.autocomplete,
        input.aria_label, input.associated_label, input.surrounding_text,
    )
    .to_lowercase();

    // Check for password pattern matches in breakdown
    let has_password_signal = breakdown.iter().any(|(desc, _)| desc.contains("password"));
    if has_password_signal {
        return FieldRole::Password;
    }

    // Phone detection
    if input.input_type == "tel" || input.autocomplete == "tel" {
        return FieldRole::Phone;
    }
    if PHONE_PATTERNS.iter().any(|p| all_text.contains(p)) {
        return FieldRole::Phone;
    }

    // Email detection
    if input.input_type == "email" || input.autocomplete == "email" {
        return FieldRole::Email;
    }
    if EMAIL_PATTERNS.iter().any(|p| all_text.contains(p)) {
        return FieldRole::Email;
    }

    // Username detection
    if input.autocomplete == "username" {
        return FieldRole::Username;
    }
    if USERNAME_PATTERNS.iter().any(|p| all_text.contains(p)) {
        return FieldRole::Username;
    }

    // If it's a text input in a form with a password field, likely username
    if input.input_type == "text" {
        return FieldRole::Username;
    }

    FieldRole::Unknown
}

/// Infer the IdentifierType from a username-role input's contextual clues.
fn infer_identifier_type(input: &InputInfo) -> IdentifierType {
    let all_text = format!(
        "{} {} {} {}",
        input.placeholder, input.aria_label, input.associated_label, input.surrounding_text,
    )
    .to_lowercase();

    // Check for combined hints like "Username or email"
    let has_email = EMAIL_PATTERNS.iter().any(|p| all_text.contains(p));
    let has_user = USERNAME_PATTERNS.iter().any(|p| all_text.contains(p));
    let has_phone = PHONE_PATTERNS.iter().any(|p| all_text.contains(p));

    if has_user && has_email {
        IdentifierType::UsernameOrEmail
    } else if has_email {
        IdentifierType::Email
    } else if has_phone {
        IdentifierType::Phone
    } else if has_user {
        IdentifierType::Username
    } else {
        IdentifierType::Unknown
    }
}

// ─── Button Classification ─────────────────────────────────────────────

/// Score a button for its role in the login flow.
fn score_button(button: &ButtonInfo) -> ScoredButton {
    let mut breakdown: Vec<(String, f64)> = Vec::new();
    let mut total: f64 = 0.0;

    let text = button.text.to_lowercase();
    let aria = button.aria_label.to_lowercase();
    let combined = format!("{text} {aria}");

    // Signal 1: text matches login patterns (weight: 0.35)
    if LOGIN_BUTTON_PATTERNS.iter().any(|p| combined.contains(p)) {
        breakdown.push(("text matches login pattern".into(), 0.35));
        total += 0.35;
    }

    // Signal 2: type=submit (weight: 0.20)
    if button.button_type == "submit" {
        breakdown.push(("type=submit".into(), 0.20));
        total += 0.20;
    }

    // Signal 3: form association (weight: 0.20)
    if button.form_index.is_some() {
        breakdown.push(("in form".into(), 0.20));
        total += 0.20;
    }

    // Signal 4: ARIA label (weight: 0.15)
    if !aria.is_empty() && LOGIN_BUTTON_PATTERNS.iter().any(|p| aria.contains(p)) {
        breakdown.push(("aria-label matches login".into(), 0.15));
        total += 0.15;
    }

    // Determine role
    let role = if METHOD_SELECTOR_PATTERNS.iter().any(|p| combined.contains(p)) {
        ButtonRole::MethodSelector
    } else if CONTINUE_BUTTON_PATTERNS.iter().any(|p| combined.contains(p)) {
        ButtonRole::Continue
    } else if LOGIN_BUTTON_PATTERNS.iter().any(|p| combined.contains(p)) || button.button_type == "submit" {
        ButtonRole::Login
    } else {
        ButtonRole::Other
    };

    ScoredButton {
        button: button.clone(),
        confidence: total.min(1.0),
        role,
        score_breakdown: breakdown,
    }
}

// ─── Logging Helpers ────────────────────────────────────────────────────

fn log_field_score(logger: &SmartLoginLogger, field: &ScoredField) {
    let desc = format!(
        "input#{} (name={}, type={})",
        if !field.input.id.is_empty() { &field.input.id } else { "?" },
        if !field.input.name.is_empty() { &field.input.name } else { "?" },
        field.input.input_type,
    );

    logger.emit(
        SmartLoginEvent::new(
            LoginState::FormDetected,
            format!(
                "{desc} → {:?} (confidence: {:.2})",
                field.role, field.confidence,
            ),
        )
        .with_detail(SmartLoginEventDetail::FieldScored {
            field_description: desc,
            role: format!("{:?}", field.role),
            confidence: field.confidence,
            breakdown: field.score_breakdown.clone(),
        }),
    );
}

fn log_button_score(logger: &SmartLoginLogger, button: &ScoredButton) {
    let desc = format!("\"{}\"", button.button.text);
    logger.emit(
        SmartLoginEvent::new(
            LoginState::FormDetected,
            format!(
                "Button {desc} → {:?} (confidence: {:.2})",
                button.role, button.confidence,
            ),
        )
        .with_detail(SmartLoginEventDetail::FieldScored {
            field_description: desc,
            role: format!("{:?}", button.role),
            confidence: button.confidence,
            breakdown: button.score_breakdown.clone(),
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_input(input_type: &str, name: &str, autocomplete: &str, placeholder: &str) -> InputInfo {
        InputInfo {
            backend_node_id: 0,
            input_type: input_type.into(),
            name: name.into(),
            id: name.into(),
            placeholder: placeholder.into(),
            autocomplete: autocomplete.into(),
            aria_label: String::new(),
            associated_label: String::new(),
            is_visible: true,
            is_readonly: false,
            form_index: Some(0),
            surrounding_text: String::new(),
            ax_role: "textbox".into(),
            ax_name: String::new(),
        }
    }

    #[test]
    fn test_password_field_high_confidence() {
        let input = make_input("password", "password", "current-password", "Password");
        let scored = score_input(&input);
        assert_eq!(scored.role, FieldRole::Password);
        assert!(scored.confidence > 0.50, "Password confidence {:.2} too low", scored.confidence);
    }

    #[test]
    fn test_username_field() {
        let input = make_input("text", "username", "username", "Username");
        let scored = score_input(&input);
        assert_eq!(scored.role, FieldRole::Username);
        assert!(scored.confidence > 0.40, "Username confidence {:.2} too low", scored.confidence);
    }

    #[test]
    fn test_email_field() {
        let input = make_input("email", "email", "email", "Email address");
        let scored = score_input(&input);
        assert_eq!(scored.role, FieldRole::Email);
        assert!(scored.confidence > 0.40, "Email confidence {:.2} too low", scored.confidence);
    }

    #[test]
    fn test_phone_field() {
        let input = make_input("tel", "phone", "tel", "Phone number");
        let scored = score_input(&input);
        assert_eq!(scored.role, FieldRole::Phone);
        assert!(scored.confidence > 0.40, "Phone confidence {:.2} too low", scored.confidence);
    }

    #[test]
    fn test_submit_button_scored() {
        let button = ButtonInfo {
            backend_node_id: 0,
            text: "Sign in".into(),
            button_type: "submit".into(),
            aria_label: String::new(),
            is_visible: true,
            form_index: Some(0),
            ax_role: "button".into(),
            ax_name: String::new(),
        };
        let scored = score_button(&button);
        assert_eq!(scored.role, ButtonRole::Login);
        assert!(scored.confidence > 0.60, "Login button confidence {:.2} too low", scored.confidence);
    }

    #[test]
    fn test_continue_button() {
        let button = ButtonInfo {
            backend_node_id: 0,
            text: "Continue".into(),
            button_type: "button".into(),
            aria_label: String::new(),
            is_visible: true,
            form_index: Some(0),
            ax_role: "button".into(),
            ax_name: String::new(),
        };
        let scored = score_button(&button);
        assert_eq!(scored.role, ButtonRole::Continue);
    }
}
