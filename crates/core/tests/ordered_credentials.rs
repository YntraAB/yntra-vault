use yntra_vault_core::vault::{VaultManager, NewEntry, UpdateEntry, CustomField, FieldType};
use uuid::Uuid;

fn field(kind: FieldType, name: &str, value: &str) -> CustomField {
    CustomField { id: Uuid::new_v4(), name: name.into(), field_type: kind, value: value.into(), sensitive: true }
}

#[test]
fn ordered_credentials_and_tag_defaults_survive_storage_and_failed_writes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fixture.vdb");
    let master = "Synthetic ordered fields master";
    let mut vault = VaultManager::create("Fixture", master, &path).unwrap();
    let tag = vault.add_tag_with_url("Mail", "blue", "tag", "gmail.com").unwrap();
    assert_eq!(vault.tags()[0].default_url, "https://gmail.com/");
    let phone = field(FieldType::Phone, "Phone", "+46700000000");
    let second_password = field(FieldType::Password, "Second password", "secondary-password");
    let second_user = field(FieldType::Username, "Second username", "secondary-user");
    let url = field(FieldType::Url, "Other website", "https://example.test/login");
    let codes = field(FieldType::Password, "2FA Recovery Codes", "not-a-password");
    let order = field(FieldType::Text, "_field_order", &format!("{},{},{},{},{},username,password", codes.id, phone.id, second_user.id, second_password.id, url.id));
    let id = vault.add_entry(NewEntry { title: "Synthetic".into(), username: "original-user".into(), password: "original-password".into(), email: "original@example.test".into(), url: "https://original.test".into(), notes: String::new(), tags: vec!["Mail".into()], totp_secret: None, custom_fields: vec![phone.clone(), second_password.clone(), second_user.clone(), url, codes, order.clone()], entry_type: None, generate_passkey: None, attachments: None }).unwrap();
    let mut reopened = VaultManager::open(&path, master).unwrap();
    let entry = reopened.get_entry(id).unwrap();
    let selected = entry.login_fields(true);
    assert_eq!(selected.identifier, "+46700000000");
    assert_eq!(selected.password, "secondary-password");
    assert_eq!(selected.url, "https://example.test/login");
    let mut fields = entry.custom_fields.clone();
    fields.iter_mut().find(|f| f.id == order.id).unwrap().value = format!("{},username,password,{}", second_user.id, second_password.id);
    reopened.update_entry(id, UpdateEntry { custom_fields: Some(fields), ..Default::default() }).unwrap();
    let reordered = reopened.get_entry(id).unwrap();
    assert_eq!(reordered.login_fields(false).identifier, "secondary-user");
    assert_eq!(reordered.login_fields(false).password, "original-password");
    assert!(vault.update_tag_with_url(tag, "Mail", "blue", "tag", Some("https://changed.test")).is_err());
    assert_eq!(vault.tags()[0].default_url, "https://gmail.com/");
    for invalid in ["javascript://alert(1)", "file:///tmp/test", "https://user:password@example.test", "https://example.test/\nnext"] {
        assert!(reopened.update_tag_with_url(tag, "Mail", "blue", "tag", Some(invalid)).is_err());
    }
    reopened.update_tag(tag, "Renamed", "blue", "tag").unwrap();
    assert_eq!(reopened.tags()[0].default_url, "https://gmail.com/");
    assert_eq!(reopened.get_entry(id).unwrap().url, "https://original.test");
    reopened.update_tag_with_url(tag, "Renamed", "blue", "tag", Some("")).unwrap();
    assert!(VaultManager::open(&path, master).unwrap().tags()[0].default_url.is_empty());
}
