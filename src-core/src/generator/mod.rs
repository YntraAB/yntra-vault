//! Cryptographically secure password generator
//!
//! Two modes:
//! 1. Random — configurable character set + length
//! 2. Diceware — memorable passphrases using word lists

use rand::Rng;
use serde::{Deserialize, Serialize};

/// Options for password generation.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GeneratorOptions {
    pub mode: GeneratorMode,
    pub length: usize,
    pub uppercase: bool,
    pub lowercase: bool,
    pub digits: bool,
    pub symbols: bool,
    pub exclude_ambiguous: bool,
    pub custom_symbols: Option<String>,
    pub word_count: usize,
    pub separator: String,
    pub capitalize_words: bool,
    pub add_number: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum GeneratorMode {
    Random,
    Diceware,
}

impl Default for GeneratorOptions {
    fn default() -> Self {
        GeneratorOptions {
            mode: GeneratorMode::Random,
            length: 20,
            uppercase: true,
            lowercase: true,
            digits: true,
            symbols: true,
            exclude_ambiguous: false,
            custom_symbols: None,
            word_count: 5,
            separator: "-".to_string(),
            capitalize_words: true,
            add_number: true,
        }
    }
}

const UPPERCASE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const LOWERCASE: &str = "abcdefghijklmnopqrstuvwxyz";
const DIGITS: &str = "0123456789";
const SYMBOLS: &str = "!@#$%^&*()_+-=[]{}|;:',.<>?/~`";
const UPPERCASE_NO_AMBIGUOUS: &str = "ABCDEFGHJKLMNPQRSTUVWXYZ";
const LOWERCASE_NO_AMBIGUOUS: &str = "abcdefghjkmnpqrstuvwxyz";
const DIGITS_NO_AMBIGUOUS: &str = "23456789";

/// Diceware-style word list (EFF short wordlist subset).
const WORDLIST: &[&str] = &[
    "abandon", "ability", "able", "about", "above", "absent", "absorb", "abstract",
    "absurd", "abuse", "access", "accident", "account", "accuse", "achieve", "acid",
    "acoustic", "acquire", "across", "action", "actor", "actress", "actual", "adapt",
    "address", "adjust", "admit", "adult", "advance", "advice", "aerobic", "affair",
    "afford", "afraid", "again", "agent", "agree", "ahead", "alarm", "album",
    "alcohol", "alert", "alien", "almost", "alone", "alpha", "already", "also",
    "alter", "always", "amateur", "amazing", "among", "amount", "amused", "anchor",
    "ancient", "anger", "angle", "animal", "ankle", "announce", "annual", "another",
    "answer", "antenna", "antique", "anxiety", "apart", "apology", "appear", "apple",
    "approve", "april", "arctic", "area", "arena", "argue", "armor", "army",
    "arrange", "arrest", "arrive", "arrow", "art", "artefact", "artist", "assault",
    "asset", "assist", "assume", "asthma", "atom", "attack", "attend", "attract",
    "auction", "audit", "august", "aunt", "autumn", "average", "avocado", "avoid",
    "awake", "aware", "awesome", "awful", "awkward", "axis", "badge", "balance",
    "balcony", "bamboo", "banana", "banner", "barrel", "basket", "battle", "beach",
    "beauty", "because", "become", "before", "begin", "behave", "behind", "believe",
    "benefit", "best", "betray", "better", "between", "beyond", "bicycle", "bitter",
    "blanket", "bless", "blind", "blood", "blossom", "board", "bonus", "border",
    "bounce", "brain", "brand", "brave", "bread", "breeze", "brick", "bridge",
    "bright", "bring", "broken", "bronze", "brother", "brush", "bubble", "buddy",
    "budget", "buffalo", "build", "bullet", "bundle", "burger", "burst", "butter",
    "cabin", "cable", "cactus", "camera", "camp", "canal", "cancel", "candy",
    "cannon", "canvas", "canyon", "capable", "capital", "captain", "carbon", "carpet",
    "carry", "castle", "catalog", "catch", "cattle", "ceiling", "celery", "cement",
    "census", "cereal", "certain", "chain", "chair", "chalk", "champion", "change",
    "chaos", "chapter", "charge", "chase", "cherry", "chicken", "chief", "child",
    "chimney", "choice", "chunk", "circle", "citizen", "civil", "claim", "clap",
    "clarify", "claw", "clay", "clean", "clerk", "clever", "cliff", "climb",
    "clock", "close", "cloth", "cloud", "clown", "cluster", "coach", "coast",
    "coconut", "coffee", "coil", "collect", "color", "column", "combine", "comfort",
    "cosmic", "cotton", "couch", "country", "couple", "course", "cousin", "cover",
    "crystal", "cube", "culture", "cupboard", "curious", "current", "curtain", "cycle",
    "damage", "dance", "danger", "daring", "dash", "daughter", "dawn", "debate",
    "desert", "design", "detail", "detect", "develop", "device", "diamond", "diary",
    "digital", "dinner", "dinosaur", "direct", "discover", "disease", "display", "doctor",
    "document", "domain", "donate", "door", "double", "dragon", "drama", "dream",
    "drift", "drill", "drink", "drive", "drop", "drum", "duck", "dumb",
    "eagle", "early", "earn", "earth", "easily", "east", "echo", "ecology",
    "economy", "edge", "edit", "effort", "eight", "either", "elbow", "elder",
    "electric", "elegant", "element", "elephant", "elite", "embrace", "emerge", "emotion",
    "enable", "endure", "energy", "engine", "enhance", "enjoy", "enough", "entire",
    "envelope", "episode", "equal", "error", "escape", "estate", "eternal", "evidence",
    "evil", "evolve", "exact", "example", "excess", "exchange", "excite", "exercise",
    "exhaust", "exile", "exist", "expand", "expect", "expire", "explain", "express",
    "fabric", "faculty", "faint", "faith", "family", "famous", "fancy", "fantasy",
    "fashion", "father", "fault", "favorite", "feature", "fence", "festival", "fever",
    "fiction", "field", "figure", "film", "filter", "final", "finger", "finish",
    "fire", "first", "fish", "fitness", "flag", "flame", "flash", "flavor",
    "flight", "float", "floor", "flower", "fluid", "focus", "follow", "force",
    "forest", "forget", "fortune", "forward", "fossil", "found", "frame", "fresh",
    "friend", "frozen", "fruit", "fuel", "funny", "future", "gadget", "galaxy",
    "gallery", "game", "garage", "garden", "garlic", "gather", "gentle", "genuine",
    "ghost", "giant", "gift", "giggle", "giraffe", "glad", "glance", "glass",
    "globe", "glory", "glove", "glow", "gold", "good", "gorilla", "gospel",
    "govern", "grace", "grain", "grape", "grass", "gravity", "great", "green",
    "grocery", "group", "grow", "guard", "guess", "guide", "guitar", "habit",
    "half", "hammer", "happy", "harbor", "harvest", "health", "heart", "heavy",
    "height", "hero", "hidden", "highway", "hint", "history", "hobby", "hockey",
    "holiday", "hollow", "home", "honey", "hope", "horror", "horse", "hospital",
    "host", "hour", "hover", "huge", "human", "humble", "humor", "hundred",
    "hungry", "hurdle", "husband", "hybrid", "icon", "idea", "identify", "idle",
    "ignore", "image", "immune", "impact", "improve", "impulse", "include", "income",
    "index", "indoor", "industry", "infant", "inform", "initial", "inner", "innocent",
    "input", "insane", "insect", "inside", "inspire", "install", "intact", "interest",
    "involve", "iron", "island", "isolate", "issue", "ivory", "jacket",
];

/// Generate a password based on the given options.
pub fn generate_password(options: &GeneratorOptions) -> String {
    match options.mode {
        GeneratorMode::Random => generate_random(options),
        GeneratorMode::Diceware => generate_diceware(options),
    }
}

/// Generate a random password from the configured character set.
fn generate_random(options: &GeneratorOptions) -> String {
    let mut charset = String::new();

    if options.uppercase {
        charset.push_str(if options.exclude_ambiguous { UPPERCASE_NO_AMBIGUOUS } else { UPPERCASE });
    }
    if options.lowercase {
        charset.push_str(if options.exclude_ambiguous { LOWERCASE_NO_AMBIGUOUS } else { LOWERCASE });
    }
    if options.digits {
        charset.push_str(if options.exclude_ambiguous { DIGITS_NO_AMBIGUOUS } else { DIGITS });
    }
    if options.symbols {
        if let Some(ref custom) = options.custom_symbols {
            charset.push_str(custom);
        } else {
            charset.push_str(SYMBOLS);
        }
    }

    if charset.is_empty() {
        charset.push_str(LOWERCASE);
        charset.push_str(DIGITS);
    }

    let chars: Vec<char> = charset.chars().collect();
    let mut rng = rand::rng();

    let mut password: Vec<char> = (0..options.length)
        .map(|_| chars[rng.random_range(0..chars.len())])
        .collect();

    // Ensure at least one character from each enabled category
    let mut pos = 0;
    if options.uppercase && !password.iter().any(|c| c.is_uppercase()) && pos < options.length {
        let upper_chars: Vec<char> = if options.exclude_ambiguous { UPPERCASE_NO_AMBIGUOUS } else { UPPERCASE }.chars().collect();
        password[pos] = upper_chars[rng.random_range(0..upper_chars.len())];
        pos += 1;
    }
    if options.lowercase && !password.iter().any(|c| c.is_lowercase()) && pos < options.length {
        let lower_chars: Vec<char> = if options.exclude_ambiguous { LOWERCASE_NO_AMBIGUOUS } else { LOWERCASE }.chars().collect();
        password[pos] = lower_chars[rng.random_range(0..lower_chars.len())];
        pos += 1;
    }
    if options.digits && !password.iter().any(|c| c.is_ascii_digit()) && pos < options.length {
        let digit_chars: Vec<char> = if options.exclude_ambiguous { DIGITS_NO_AMBIGUOUS } else { DIGITS }.chars().collect();
        password[pos] = digit_chars[rng.random_range(0..digit_chars.len())];
        pos += 1;
    }
    if options.symbols && !password.iter().any(|c| SYMBOLS.contains(*c)) && pos < options.length {
        let sym_chars: Vec<char> = SYMBOLS.chars().collect();
        password[pos] = sym_chars[rng.random_range(0..sym_chars.len())];
    }

    // Fisher-Yates shuffle
    for i in (1..password.len()).rev() {
        let j = rng.random_range(0..=i);
        password.swap(i, j);
    }

    password.into_iter().collect()
}

/// Generate a diceware-style passphrase.
fn generate_diceware(options: &GeneratorOptions) -> String {
    let mut rng = rand::rng();
    let word_count = if options.word_count < 3 { 5 } else { options.word_count };

    let mut words: Vec<String> = (0..word_count)
        .map(|_| {
            let word = WORDLIST[rng.random_range(0..WORDLIST.len())].to_string();
            if options.capitalize_words {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                }
            } else {
                word
            }
        })
        .collect();

    if options.add_number {
        let num = rng.random_range(0..1000u32);
        let insert_pos = rng.random_range(0..=words.len());
        words.insert(insert_pos, num.to_string());
    }

    words.join(&options.separator)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_password_length() {
        let options = GeneratorOptions {
            mode: GeneratorMode::Random,
            length: 32,
            ..Default::default()
        };
        let pw = generate_password(&options);
        assert_eq!(pw.len(), 32);
    }

    #[test]
    fn test_random_password_uniqueness() {
        let options = GeneratorOptions::default();
        let pw1 = generate_password(&options);
        let pw2 = generate_password(&options);
        assert_ne!(pw1, pw2);
    }

    #[test]
    fn test_random_password_has_all_types() {
        let options = GeneratorOptions {
            mode: GeneratorMode::Random,
            length: 32,
            uppercase: true,
            lowercase: true,
            digits: true,
            symbols: true,
            ..Default::default()
        };

        for _ in 0..10 {
            let pw = generate_password(&options);
            assert!(pw.chars().any(|c| c.is_uppercase()), "Missing uppercase in: {}", pw);
            assert!(pw.chars().any(|c| c.is_lowercase()), "Missing lowercase in: {}", pw);
            assert!(pw.chars().any(|c| c.is_ascii_digit()), "Missing digit in: {}", pw);
            assert!(pw.chars().any(|c| SYMBOLS.contains(c)), "Missing symbol in: {}", pw);
        }
    }

    #[test]
    fn test_diceware_word_count() {
        let options = GeneratorOptions {
            mode: GeneratorMode::Diceware,
            word_count: 5,
            separator: "-".to_string(),
            capitalize_words: false,
            add_number: false,
            ..Default::default()
        };

        let pw = generate_password(&options);
        let word_count = pw.split('-').count();
        assert_eq!(word_count, 5);
    }

    #[test]
    fn test_diceware_with_number() {
        let options = GeneratorOptions {
            mode: GeneratorMode::Diceware,
            word_count: 4,
            separator: "-".to_string(),
            add_number: true,
            capitalize_words: false,
            ..Default::default()
        };

        let pw = generate_password(&options);
        let parts: Vec<&str> = pw.split('-').collect();
        assert_eq!(parts.len(), 5);
        assert!(parts.iter().any(|p| p.parse::<u32>().is_ok()));
    }

    #[test]
    fn test_exclude_ambiguous() {
        let options = GeneratorOptions {
            mode: GeneratorMode::Random,
            length: 1000,
            exclude_ambiguous: true,
            uppercase: true,
            lowercase: true,
            digits: true,
            symbols: false,
            ..Default::default()
        };

        let pw = generate_password(&options);
        assert!(!pw.contains('O'));
        assert!(!pw.contains('0'));
        assert!(!pw.contains('l'));
        assert!(!pw.contains('1'));
        assert!(!pw.contains('I'));
    }
}

