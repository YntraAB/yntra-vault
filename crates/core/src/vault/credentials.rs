use super::{entry::DecryptedEntry, types::FieldType};

pub struct LoginFields<'a> {
    pub identifier: &'a str,
    pub password: &'a str,
    pub url: &'a str,
}

impl DecryptedEntry {
    /// Resolve the same order shown in the editor. Removed IDs and empty fields
    /// do not hide usable fields added by another replica.
    pub fn login_fields(&self, legacy_prefer_email: bool) -> LoginFields<'_> {
        let layout = self.custom_fields.iter().find(|f| f.name == "_field_order");
        let mut candidates: Vec<(FieldType, &str)> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let fallback = if layout.is_none() && legacy_prefer_email {
            ["email", "username", "password", "url"]
        } else { ["username", "email", "password", "url"] };
        let custom_ids: Vec<String> = self.custom_fields.iter().map(|f| f.id.to_string()).collect();
        for id in layout.into_iter().flat_map(|f| f.value.split(',').map(str::trim))
            .chain(fallback).chain(custom_ids.iter().map(String::as_str)) {
            if !seen.insert(id) { continue; }
            let candidate = match id {
                "username" => Some((FieldType::Username, self.username.as_str())),
                "email" => Some((FieldType::Email, self.email.as_str())),
                "password" => Some((FieldType::Password, self.password.as_str())),
                "url" => Some((FieldType::Url, self.url.as_str())),
                _ => self.custom_fields.iter().find(|f| f.id.to_string() == id
                    && !f.name.starts_with('_') && !is_recovery_field(&f.name))
                    .map(|f| (f.field_type.clone(), f.value.as_str())),
            };
            if let Some((kind, value)) = candidate {
                if !value.trim().is_empty() { candidates.push((kind, value)); }
            }
        }
        LoginFields {
            identifier: candidates.iter().find(|(kind, _)| matches!(kind, FieldType::Username | FieldType::Email | FieldType::Phone)).map(|(_, value)| *value).unwrap_or_default(),
            password: candidates.iter().find(|(kind, _)| *kind == FieldType::Password).map(|(_, value)| *value).unwrap_or_default(),
            url: candidates.iter().find(|(kind, _)| *kind == FieldType::Url).map(|(_, value)| *value).unwrap_or_default(),
        }
    }
}

fn is_recovery_field(name: &str) -> bool {
    let name = name.trim().to_lowercase();
    matches!(name.as_str(), "2fa recovery codes" | "2fa recovery code" | "2fa backup codes" | "recovery codes" | "recovery code" | "backup codes") || name.starts_with("2fa recovery cod")
}
