//! Field revisions and reviewed reconciliation shared by synchronization transports.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

use crate::{
    Result, VaultError,
    crypto::{SubKeys, cipher::EncryptedBlob},
    vault::{Entry, FieldScope, VaultData, VaultManager},
};

pub const SYNC_METADATA_VERSION: u8 = 2;

pub type Clock = BTreeMap<Uuid, u64>;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SyncMetadata {
    pub version: u8,
    pub clock: Clock,
    pub fields: BTreeMap<String, Revision>,
    pub history: BTreeMap<Uuid, Vec<ConflictVersion>>,
    #[serde(default)]
    pub receipts: Vec<Uuid>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ConflictVersion {
    pub id: Uuid,
    pub saved_at: DateTime<Utc>,
    pub encrypted_entry: EncryptedBlob,
    #[serde(default)]
    pub clock: Clock,
}

fn trim_history(history: &mut Vec<ConflictVersion>) {
    history.sort_by_key(|version| version.id);
    history.dedup_by_key(|version| version.id);
    let ranks: BTreeMap<_, _> = history.iter().map(|version| {
        (version.id, history.iter().filter(|other| dominates(&version.clock, &other.clock)).count())
    }).collect();
    history.sort_by_key(|version| (ranks[&version.id], version.saved_at, version.id));
    if history.len() > 10 {
        history.drain(..history.len() - 10);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Revision {
    pub clock: Clock,
    pub fingerprint: [u8; 32],
    pub device: Uuid,
    pub changed_at: DateTime<Utc>,
    #[serde(default)]
    pub time_reliable: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SyncPolicy {
    #[serde(default)]
    pub review_conflicts_only: bool,
    #[serde(default)]
    pub prefer_latest: bool,
}

pub fn apply_policy(preview: &mut SyncPreview, local: &SyncPolicy, remote: &SyncPolicy) -> (Vec<SyncDecision>, bool) {
    let mut decisions = Vec::new();
    let now = Utc::now();
    if local.prefer_latest {
        for change in &mut preview.changes {
            if !change.conflict || change.requires_explicit_choice || change.field == "state" || change.local_present != change.remote_present || !change.time_order_reliable { continue; }
            if let (Some(a), Some(b)) = (change.local_changed_at, change.remote_changed_at) {
                if a != b && a <= now && b <= now {
                    let choice = if a > b { Choice::Local } else { Choice::Remote };
                    change.suggested = Some(choice.clone());
                    decisions.push(SyncDecision { key: change.key.clone(), choice, text: None });
                }
            }
        }
    }
    let needs_review = !preview.changes.is_empty()
        && (!(local.review_conflicts_only && remote.review_conflicts_only)
            || preview.changes.iter().any(|change| change.conflict
                && (!remote.prefer_latest || !decisions.iter().any(|decision| decision.key == change.key))));
    (decisions, needs_review)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncPreview {
    pub id: Uuid,
    #[serde(default)]
    pub background: bool,
    pub local_revision: String,
    pub remote_revision: String,
    pub changes: Vec<FieldChange>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FieldChange {
    pub key: String,
    pub entry_id: Uuid,
    pub title: String,
    pub field: String,
    pub custom_field_name: Option<String>,
    pub conflict: bool,
    pub suggested: Option<Choice>,
    pub local_changed_at: Option<DateTime<Utc>>,
    pub remote_changed_at: Option<DateTime<Utc>>,
    pub local_device: Option<Uuid>,
    pub remote_device: Option<Uuid>,
    pub local_device_name: Option<String>,
    pub remote_device_name: Option<String>,
    pub local_present: bool,
    pub remote_present: bool,
    pub sensitive: bool,
    pub can_keep_both: bool,
    pub can_edit: bool,
    pub time_order_reliable: bool,
    #[serde(default)]
    pub requires_explicit_choice: bool,
    pub keep_both_entries: Option<[ResultIdentity; 2]>,
    pub local_state: Option<String>,
    pub remote_state: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResultIdentity {
    pub id: Uuid,
    pub title: String,
}

fn result_title(title: &str, suffix: &str) -> String {
    let available =
        crate::vault::validation::MAX_DISPLAY_NAME_LENGTH - suffix.encode_utf16().count();
    let mut length = 0;
    let prefix: String = title
        .chars()
        .take_while(|c| {
            length += c.len_utf16();
            length <= available
        })
        .collect();
    format!("{prefix}{suffix}")
}

fn copy_identity(operation: Uuid, source: Uuid) -> Uuid {
    let mut hash = blake3::Hasher::new_derive_key("yntra-sync-duplicate-identity-v1");
    hash.update(operation.as_bytes());
    hash.update(source.as_bytes());
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&hash.finalize().as_bytes()[..16]);
    uuid::Builder::from_random_bytes(bytes).into_uuid()
}

fn validate_version(data: &VaultData) -> Result<()> {
    if data.settings.sync.version > SYNC_METADATA_VERSION {
        return Err(fail(
            "Update this app to read the vault's synchronization metadata",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Choice {
    Local,
    Remote,
    Both,
    Edited,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncDecision {
    pub key: String,
    pub choice: Choice,
    pub text: Option<String>,
}

impl Drop for SyncDecision {
    fn drop(&mut self) {
        self.text.zeroize();
    }
}

fn fail(message: &str) -> VaultError {
    VaultError::SyncError(message.into())
}

pub fn join(target: &mut Clock, other: &Clock) {
    for (actor, counter) in other {
        target
            .entry(*actor)
            .and_modify(|c| *c = (*c).max(*counter))
            .or_insert(*counter);
    }
}

fn dominates(a: &Clock, b: &Clock) -> bool {
    a != b
        && b.iter()
            .all(|(actor, count)| a.get(actor).copied().unwrap_or(0) >= *count)
}

fn tick(clock: &mut Clock, actor: Uuid) -> Result<()> {
    let value = clock.entry(actor).or_default();
    *value = value
        .checked_add(1)
        .ok_or_else(|| fail("Synchronization revision limit reached"))?;
    Ok(())
}

fn wipe(value: &mut Value) {
    match value {
        Value::String(text) => text.zeroize(),
        Value::Array(values) => values.iter_mut().for_each(wipe),
        Value::Object(values) => values.values_mut().for_each(wipe),
        _ => {}
    }
}

struct Fields(BTreeMap<String, Value>);
impl Drop for Fields {
    fn drop(&mut self) {
        self.0.values_mut().for_each(wipe);
    }
}

fn fields(entry: &Entry) -> Result<Fields> {
    let mut values = BTreeMap::new();
    let mut json =
        serde_json::to_value(entry).map_err(|_| fail("Cannot read synchronization fields"))?;
    for name in [
        "title",
        "username",
        "email",
        "url",
        "notes",
        "tags",
        "favorite",
        "pinned",
        "entry_type",
        "encrypted_password",
        "encrypted_totp_secret",
    ] {
        values.insert(name.into(), json[name].take());
    }
    values.insert(
        "passkey".into(),
        serde_json::json!([
            json["encrypted_passkey"].take(),
            json["passkey_public_key"].take()
        ]),
    );
    for (name, prefix) in [("custom_fields", "custom"), ("attachments", "attachment")] {
        if let Some(items) = json[name].as_array_mut() {
            for item in items {
                let id = item["id"]
                    .as_str()
                    .ok_or_else(|| fail("Missing synchronization field identity"))?
                    .to_owned();
                if values
                    .insert(format!("{prefix}/{id}"), item.take())
                    .is_some()
                {
                    wipe(&mut json);
                    return Err(fail("Duplicate synchronization field identity"));
                }
            }
        }
    }
    wipe(&mut json);
    Ok(Fields(values))
}

fn fingerprint(value: &Value, keys: &SubKeys, entry: Uuid, field: &str) -> Result<[u8; 32]> {
    let key = Zeroizing::new(blake3::derive_key(
        "yntra-sync-field-comparison-v1",
        &keys.hmac_key.bytes,
    ));
    let bytes = match field {
        "encrypted_password" | "encrypted_totp_secret" if !value.is_null() => {
            let blob =
                serde_json::from_value(value.clone()).map_err(|_| fail("Invalid secret field"))?;
            VaultManager::decrypt_entry_field(
                &blob,
                &keys.entry_key,
                &entry,
                if field == "encrypted_password" {
                    FieldScope::Password
                } else {
                    FieldScope::Totp
                },
            )?
        }
        "passkey" if !value[0].is_null() => {
            let blob = serde_json::from_value(value[0].clone())
                .map_err(|_| fail("Invalid passkey field"))?;
            let mut bytes = VaultManager::decrypt_entry_field(
                &blob,
                &keys.entry_key,
                &entry,
                FieldScope::Passkey,
            )?;
            bytes.extend_from_slice(
                &serde_json::to_vec(&value[1]).map_err(|_| fail("Invalid passkey public key"))?,
            );
            bytes
        }
        _ if field.starts_with("attachment/") && !value.is_null() => {
            let attachment: crate::vault::FileAttachment =
                serde_json::from_value(value.clone()).map_err(|_| fail("Invalid attachment"))?;
            let mut bytes = VaultManager::decrypt_entry_field(
                &attachment.encrypted_blob,
                &keys.entry_key,
                &entry,
                FieldScope::Attachment {
                    attachment_id: &attachment.id,
                },
            )?;
            bytes.extend_from_slice(
                &serde_json::to_vec(&(&attachment.name, attachment.size, &attachment.mime_type))
                    .map_err(|_| fail("Invalid attachment metadata"))?,
            );
            bytes
        }
        _ => Zeroizing::new(
            serde_json::to_vec(value).map_err(|_| fail("Cannot compare synchronization field"))?,
        ),
    };
    Ok(*blake3::keyed_hash(&key, &bytes).as_bytes())
}

fn entry<'a>(data: &'a VaultData, id: &Uuid) -> Option<&'a Entry> {
    data.entries.iter().find(|e| &e.id == id).or_else(|| {
        data.trash
            .iter()
            .find(|t| &t.entry.id == id)
            .map(|t| &t.entry)
    })
}

fn state(data: &VaultData, id: Uuid) -> &'static str {
    if data.entries.iter().any(|e| e.id == id) {
        "active"
    } else if data.trash.iter().any(|t| t.entry.id == id) {
        "trash"
    } else {
        "deleted"
    }
}

fn key(id: Uuid, field: &str) -> String {
    format!("{id}/{field}")
}

fn parts(key: &str) -> Result<(Uuid, &str)> {
    let (id, field) = key
        .split_once('/')
        .ok_or_else(|| fail("Invalid synchronization field key"))?;
    Ok((
        Uuid::parse_str(id).map_err(|_| fail("Invalid synchronization identity"))?,
        field,
    ))
}

fn all_fields(data: &VaultData) -> Result<Fields> {
    let mut result = Fields(BTreeMap::new());
    result.0.insert(
        key(data.metadata.id, "vault_name"),
        Value::String(data.metadata.name.clone()),
    );
    result.0.insert(
        key(data.metadata.id, "tag_order"),
        serde_json::to_value(data.tags.iter().map(|t| t.id).collect::<Vec<_>>())
            .map_err(|_| fail("Invalid tag order"))?,
    );
    let mut ids = BTreeSet::new();
    for item in data
        .entries
        .iter()
        .chain(data.trash.iter().map(|t| &t.entry))
    {
        if !ids.insert(item.id) {
            return Err(fail("Duplicate entry identity"));
        }
        result.0.insert(
            key(item.id, "state"),
            Value::String(state(data, item.id).into()),
        );
        let mut item_fields = fields(item)?;
        item_fields.0.remove("tags");
        for membership in tag_memberships(data, item)? {
            item_fields.0.insert(membership, Value::Bool(true));
        }
        for (field, value) in std::mem::take(&mut item_fields.0) {
            result.0.insert(key(item.id, &field), value);
        }
    }
    for field_key in data.settings.sync.fields.keys() {
        let (id, field) = parts(field_key)?;
        if field == "state" && !ids.contains(&id) {
            result
                .0
                .insert(field_key.clone(), Value::String("deleted".into()));
        }
    }
    for tag in &data.tags {
        if result
            .0
            .insert(
                key(tag.id, "tag"),
                serde_json::to_value(tag).map_err(|_| fail("Invalid tag"))?,
            )
            .is_some()
        {
            return Err(fail("Duplicate tag identity"));
        }
    }
    Ok(result)
}

fn is_tag_membership(field: &str) -> bool {
    field.starts_with("tag_membership/") || field.starts_with("tag_name/")
}

fn tag_memberships(data: &VaultData, item: &Entry) -> Result<BTreeSet<String>> {
    item.tags.iter().map(|name| {
        let mut tags = data.tags.iter().filter(|tag| tag.name == *name);
        let field = match tags.next() {
            Some(tag) => format!("tag_membership/{}", tag.id),
            None => format!("tag_name/{name}"),
        };
        if tags.next().is_some() {
            return Err(fail("Multiple tags have the same name; rename them before synchronizing"));
        }
        Ok(field)
    }).collect()
}

fn membership_name(data: &VaultData, field: &str) -> Option<String> {
    if let Some(name) = field.strip_prefix("tag_name/") {
        return Some(name.into());
    }
    let id = field.strip_prefix("tag_membership/").and_then(|id| Uuid::parse_str(id).ok())?;
    data.tags.iter().find(|tag| tag.id == id).map(|tag| tag.name.clone())
}

fn tag_clock(data: &VaultData, id: Uuid) -> Clock {
    let mut clock = Clock::new();
    let membership = format!("tag_membership/{id}");
    for (field, revision) in &data.settings.sync.fields {
        if field == &key(id, "tag") || parts(field).is_ok_and(|(_, name)| name == membership) {
            join(&mut clock, &revision.clock);
        }
    }
    clock
}

/// Runs at the storage boundary so every writer participates in revision tracking.
pub fn track(data: &mut VaultData, keys: &SubKeys, actor: Uuid, now: DateTime<Utc>) -> Result<()> {
    validate_version(data)?;
    let initializing = data.settings.sync.version == 0;
    let current = all_fields(data)?;
    let field_keys: BTreeSet<_> = current
        .0
        .keys()
        .chain(data.settings.sync.fields.keys())
        .cloned()
        .collect();
    let mut updates = Vec::new();
    for field_key in field_keys {
        let (id, field) = parts(&field_key)?;
        let hash = fingerprint(
            current.0.get(&field_key).unwrap_or(&Value::Null),
            keys,
            id,
            field,
        )?;
        if data
            .settings
            .sync
            .fields
            .get(&field_key)
            .is_none_or(|old| old.fingerprint != hash)
        {
            updates.push((field_key, hash));
        }
    }
    if !updates.is_empty() {
        tick(&mut data.settings.sync.clock, actor)?;
        let device = super::resolve_local_device_info(None).id;
        for (field_key, hash) in updates {
            let changed_at = if initializing {
                parts(&field_key)
                    .ok()
                    .and_then(|(id, _)| entry(data, &id).map(|e| e.updated_at))
                    .unwrap_or(now)
            } else {
                now
            };
            let time_reliable = !initializing
                && data
                    .settings
                    .sync
                    .fields
                    .get(&field_key)
                    .is_none_or(|old| now > old.changed_at);
            data.settings.sync.fields.insert(
                field_key,
                Revision {
                    clock: data.settings.sync.clock.clone(),
                    fingerprint: hash,
                    device,
                    changed_at,
                    time_reliable,
                },
            );
        }
    }
    data.settings.sync.version = SYNC_METADATA_VERSION;
    let deleted: Vec<_> = data
        .settings
        .sync
        .history
        .keys()
        .filter(|id| state(data, **id) == "deleted")
        .copied()
        .collect();
    for id in deleted {
        data.settings.sync.history.remove(&id);
    }
    Ok(())
}

/// Key rotation changes comparison keys, not the causal meaning of existing edits.
pub(crate) fn rehash(data: &mut VaultData, keys: &SubKeys) -> Result<()> {
    validate_version(data)?;
    let current = all_fields(data)?;
    for (field_key, revision) in &mut data.settings.sync.fields {
        let (id, field) = parts(field_key)?;
        revision.fingerprint = fingerprint(
            current.0.get(field_key).unwrap_or(&Value::Null),
            keys,
            id,
            field,
        )?;
    }
    Ok(())
}

pub fn revision(data: &VaultData) -> Result<String> {
    let mut data = data.clone();
    data.metadata.updated_at = data.metadata.created_at;
    let bytes = crate::vault::storage::serialize_payload(&data)?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

fn entity_clock(data: &VaultData, id: Uuid) -> Clock {
    let mut clock = Clock::new();
    let prefix = format!("{id}/");
    for (key, rev) in &data.settings.sync.fields {
        if key.starts_with(&prefix) {
            join(&mut clock, &rev.clock);
        }
    }
    clock
}

fn suggestion(a: Option<&Revision>, b: Option<&Revision>) -> Option<Choice> {
    match (a, b) {
        (Some(a), Some(b)) if dominates(&a.clock, &b.clock) => Some(Choice::Local),
        (Some(a), Some(b)) if dominates(&b.clock, &a.clock) => Some(Choice::Remote),
        _ => None,
    }
}

pub fn preview(local: &VaultData, remote: &VaultData, keys: &SubKeys) -> Result<SyncPreview> {
    validate_version(local)?;
    validate_version(remote)?;
    if local.metadata.id != remote.metadata.id {
        return Err(fail("Remote vault identity does not match"));
    }
    let left = all_fields(local)?;
    let right = all_fields(remote)?;
    let mut changes = Vec::new();
    let ids: BTreeSet<_> = local
        .entries
        .iter()
        .chain(remote.entries.iter())
        .chain(local.trash.iter().map(|t| &t.entry))
        .chain(remote.trash.iter().map(|t| &t.entry))
        .map(|e| e.id)
        .chain(
            local
                .settings
                .sync
                .fields
                .keys()
                .chain(remote.settings.sync.fields.keys())
                .filter_map(|k| {
                    parts(k)
                        .ok()
                        .filter(|(_, f)| *f == "state")
                        .map(|(id, _)| id)
                }),
        )
        .collect();
    let mut states = BTreeSet::new();
    for id in ids {
        if state(local, id) != state(remote, id)
            || entry(local, &id).is_none() != entry(remote, &id).is_none()
        {
            states.insert(id);
            let field_key = key(id, "state");
            let a = local.settings.sync.fields.get(&field_key);
            let b = remote.settings.sync.fields.get(&field_key);
            let ac = entity_clock(local, id);
            let bc = entity_clock(remote, id);
            let suggested = if ac.is_empty() && entry(local, &id).is_none() {
                Some(Choice::Remote)
            } else if bc.is_empty() && entry(remote, &id).is_none() {
                Some(Choice::Local)
            } else if dominates(&ac, &bc) {
                Some(Choice::Local)
            } else if dominates(&bc, &ac) {
                Some(Choice::Remote)
            } else {
                None
            };
            changes.push(change(local, remote, id, "state", a, b, suggested));
        }
    }
    for field_key in left.0.keys().chain(right.0.keys()).collect::<BTreeSet<_>>() {
        let (id, field) = parts(field_key)?;
        if field == "state" || states.contains(&id) {
            continue;
        }
        let a_value = left.0.get(field_key).unwrap_or(&Value::Null);
        let b_value = right.0.get(field_key).unwrap_or(&Value::Null);
        if fingerprint(a_value, keys, id, field)? == fingerprint(b_value, keys, id, field)? {
            continue;
        }
        let a = local.settings.sync.fields.get(field_key);
        let b = remote.settings.sync.fields.get(field_key);
        let mut suggested = suggestion(a, b).or_else(|| {
            if a.is_some() && b.is_none() && b_value.is_null() && remote.settings.sync.version > 0
            {
                Some(Choice::Local)
            } else if b.is_some()
                && a.is_none()
                && a_value.is_null()
                && local.settings.sync.version > 0
            {
                Some(Choice::Remote)
            } else {
                None
            }
        });
        if field == "tag" && a_value.is_null() != b_value.is_null() {
            let ac = tag_clock(local, id);
            let bc = tag_clock(remote, id);
            if !ac.is_empty() && !bc.is_empty() && !dominates(&ac, &bc) && !dominates(&bc, &ac) {
                suggested = None;
            }
        }
        changes.push(change(local, remote, id, field, a, b, suggested));
    }
    let mut tag_names: BTreeMap<&str, BTreeSet<Uuid>> = BTreeMap::new();
    for (data, side) in [(local, Choice::Local), (remote, Choice::Remote)] {
        for tag in &data.tags {
            let selected = changes.iter().find(|change| change.entry_id == tag.id && change.field == "tag")
                .and_then(|change| change.suggested.as_ref());
            if selected.is_none_or(|selected| *selected == side) {
                tag_names.entry(&tag.name).or_default().insert(tag.id);
            }
        }
    }
    for change in &mut changes {
        if change.field == "tag" && tag_names.values().any(|ids| ids.len() > 1 && ids.contains(&change.entry_id)) {
            change.conflict = true;
            change.suggested = None;
            change.requires_explicit_choice = true;
            change.title = format!("{} ({})", change.title, &change.entry_id.to_string()[..8]);
        }
    }
    let id = Uuid::new_v4();
    for change in &mut changes {
        if password_field(local, remote, change.entry_id, &change.field) {
            if let (Some(a), Some(b)) = (
                entry(local, &change.entry_id),
                entry(remote, &change.entry_id),
            ) {
                change.keep_both_entries = Some([
                    ResultIdentity {
                        id: a.id,
                        title: result_title(&a.title, " (this device)"),
                    },
                    ResultIdentity {
                        id: copy_identity(id, b.id),
                        title: result_title(&b.title, " (other device)"),
                    },
                ]);
            }
        }
    }
    Ok(SyncPreview {
        id,
        background: false,
        local_revision: revision(local)?,
        remote_revision: revision(remote)?,
        changes,
    })
}

fn change(
    local: &VaultData,
    remote: &VaultData,
    id: Uuid,
    field: &str,
    a: Option<&Revision>,
    b: Option<&Revision>,
    suggested: Option<Choice>,
) -> FieldChange {
    let present = |data: &VaultData| {
        if field == "tag" { return data.tags.iter().any(|tag| tag.id == id); }
        if field == "vault_name" || field == "tag_order" { return data.metadata.id == id; }
        let Some(item) = entry(data, &id) else { return false; };
        if let Some(field_id) = field.strip_prefix("custom/") { return item.custom_fields.iter().any(|field| field.id.to_string() == field_id); }
        if let Some(field_id) = field.strip_prefix("attachment/") { return item.attachments.iter().any(|field| field.id.to_string() == field_id); }
        if field == "encrypted_totp_secret" { return item.encrypted_totp_secret.is_some(); }
        if field == "passkey" { return item.encrypted_passkey.is_some(); }
        true
    };
    let device_name = |revision: Option<&Revision>| {
        revision.map(|revision| {
            super::identity::device_name(&local.settings.trusted_devices, revision.device)
                .or_else(|| super::identity::device_name(&remote.settings.trusted_devices, revision.device))
                .unwrap_or_else(|| revision.device.to_string())
        })
    };
    let status = |data: &VaultData| {
        if entry(data, &id).is_none() && !data.settings.sync.fields.contains_key(&key(id, "state"))
        {
            "absent"
        } else {
            state(data, id)
        }
        .to_owned()
    };
    let title = entry(local, &id)
        .or_else(|| entry(remote, &id))
        .map(|e| e.title.clone())
        .or_else(|| {
            local
                .tags
                .iter()
                .chain(remote.tags.iter())
                .find(|t| t.id == id)
                .map(|t| t.name.clone())
        })
        .unwrap_or_else(|| {
            if id == local.metadata.id {
                local.metadata.name.clone()
            } else {
                String::new()
            }
        });
    FieldChange {
        key: key(id, field),
        entry_id: id,
        title,
        field: field.into(),
        custom_field_name: field.strip_prefix("custom/").and_then(|field_id| {
            entry(local, &id).into_iter().chain(entry(remote, &id))
                .flat_map(|entry| &entry.custom_fields)
                .find(|field| field.id.to_string() == field_id)
                .map(|field| field.name.clone())
        }).or_else(|| membership_name(local, field).or_else(|| membership_name(remote, field))),
        conflict: suggested.is_none(),
        suggested,
        local_changed_at: a.map(|r| r.changed_at),
        remote_changed_at: b.map(|r| r.changed_at),
        local_device: a.map(|r| r.device),
        remote_device: b.map(|r| r.device),
        local_device_name: device_name(a),
        remote_device_name: device_name(b),
        local_present: present(local),
        remote_present: present(remote),
        sensitive: true,
        requires_explicit_choice: field == "state" || present(local) != present(remote),
        can_keep_both: field == "notes" || password_field(local, remote, id, field),
        local_state: if field == "state" { Some(status(local)) } else if present(local) != present(remote) { Some(if present(local) { "active" } else { "absent" }.into()) } else { None },
        remote_state: if field == "state" { Some(status(remote)) } else if present(local) != present(remote) { Some(if present(remote) { "active" } else { "absent" }.into()) } else { None },
        can_edit: matches!(
            field,
            "title" | "username" | "email" | "url" | "notes" | "vault_name" | "tag"
        ),
        keep_both_entries: None,
        time_order_reliable: match (a, b) {
            (Some(a), Some(b)) => {
                a.time_reliable
                    && b.time_reliable
                    && a.changed_at != b.changed_at
                    && a.changed_at <= Utc::now()
                    && b.changed_at <= Utc::now()
                    && a.clock.keys().any(|actor| b.clock.contains_key(actor))
            }
            _ => false,
        },
    }
}

fn password_field(local: &VaultData, remote: &VaultData, id: Uuid, field: &str) -> bool {
    field == "encrypted_password" || field.strip_prefix("custom/").is_some_and(|field_id| {
        entry(local, &id).into_iter().chain(entry(remote, &id)).flat_map(|item| &item.custom_fields)
            .any(|custom| custom.id.to_string() == field_id && custom.field_type == crate::vault::FieldType::Password)
    })
}

pub fn inspect(data: &VaultData, id: Uuid, field: &str, keys: &SubKeys) -> Result<String> {
    if is_tag_membership(field) {
        return Ok(match entry(data, &id) {
            Some(item) if tag_memberships(data, item)?.contains(field) => membership_name(data, field).unwrap_or_default(),
            _ => String::new(),
        });
    }
    if id == data.metadata.id && field == "vault_name" {
        return Ok(data.metadata.name.clone());
    }
    if id == data.metadata.id && field == "tag_order" {
        return Ok(data
            .tags
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>()
            .join("\n"));
    }
    if field == "state" {
        let mut summary = state(data, id).to_owned();
        if let Some(item) = entry(data, &id) {
            for name in fields(item)?.0.keys() {
                summary.push_str(&format!("\n{name}: {}", inspect(data, id, name, keys)?));
            }
        }
        return Ok(summary);
    }
    if field == "tag" {
        return Ok(data
            .tags
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.name.clone())
            .unwrap_or_default());
    }
    let Some(item) = entry(data, &id) else {
        return Ok(String::new());
    };
    if matches!(field, "encrypted_password" | "encrypted_totp_secret") {
        let blob = if field == "encrypted_password" {
            Some(&item.encrypted_password)
        } else {
            item.encrypted_totp_secret.as_ref()
        };
        return blob
            .map(|blob| {
                VaultManager::decrypt_entry_field(
                    blob,
                    &keys.entry_key,
                    &id,
                    if field == "encrypted_password" {
                        FieldScope::Password
                    } else {
                        FieldScope::Totp
                    },
                )
                .and_then(|bytes| {
                    String::from_utf8(bytes.to_vec()).map_err(|_| fail("Invalid text field"))
                })
            })
            .transpose()
            .map(|v| v.unwrap_or_default());
    }
    if field == "passkey" {
        return Ok(if item.encrypted_passkey.is_some() {
            "Passkey present"
        } else {
            "No passkey"
        }
        .into());
    }
    let values = fields(item)?;
    let value = values.0.get(field).unwrap_or(&Value::Null);
    if field.starts_with("attachment/") {
        return Ok(value["name"].as_str().unwrap_or("").into());
    }
    Ok(value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string()))
}

fn apply_field(target: &mut Entry, field: &str, value: Value) -> Result<()> {
    let mut json =
        serde_json::to_value(&*target).map_err(|_| fail("Cannot update synchronized entry"))?;
    if field == "passkey" {
        json["encrypted_passkey"] = value[0].clone();
        json["passkey_public_key"] = value[1].clone();
    } else if let Some((prefix, id)) = field.split_once('/') {
        let name = match prefix {
            "custom" => "custom_fields",
            "attachment" => "attachments",
            _ => {
                wipe(&mut json);
                return Err(fail("Unknown synchronization field"));
            }
        };
        if let Some(items) = json[name].as_array_mut() {
            items.retain(|v| v["id"].as_str() != Some(id));
            if !value.is_null() {
                items.push(value);
            }
        }
    } else {
        json[field] = value;
    }
    let replacement =
        serde_json::from_value(json.clone()).map_err(|_| fail("Invalid synchronized entry"));
    wipe(&mut json);
    *target = replacement?;
    Ok(())
}

fn duplicate(source: &Entry, keys: &SubKeys, id: Uuid) -> Result<Entry> {
    let mut copy = source.clone();
    let rewrap = |blob: &EncryptedBlob, scope: FieldScope<'_>| -> Result<EncryptedBlob> {
        let bytes = VaultManager::decrypt_entry_field(blob, &keys.entry_key, &source.id, scope)?;
        VaultManager::encrypt_entry_field(&bytes, &keys.entry_key, &id, scope)
    };
    copy.encrypted_password = rewrap(&source.encrypted_password, FieldScope::Password)?;
    copy.encrypted_totp_secret = source
        .encrypted_totp_secret
        .as_ref()
        .map(|b| rewrap(b, FieldScope::Totp))
        .transpose()?;
    copy.encrypted_passkey = source
        .encrypted_passkey
        .as_ref()
        .map(|b| rewrap(b, FieldScope::Passkey))
        .transpose()?;
    for item in &mut copy.password_history {
        let bytes = VaultManager::decrypt_entry_field(
            &item.encrypted_password,
            &keys.entry_key,
            &source.id,
            FieldScope::History,
        )
        .or_else(|_| {
            VaultManager::decrypt_entry_field(
                &item.encrypted_password,
                &keys.entry_key,
                &source.id,
                FieldScope::Password,
            )
        })?;
        item.encrypted_password =
            VaultManager::encrypt_entry_field(&bytes, &keys.entry_key, &id, FieldScope::History)?;
    }
    for item in &mut copy.attachments {
        item.encrypted_blob = rewrap(
            &item.encrypted_blob,
            FieldScope::Attachment {
                attachment_id: &item.id,
            },
        )?;
    }
    copy.id = id;
    Ok(copy)
}

pub fn resolve(
    local: &VaultData,
    remote: &VaultData,
    preview: &SyncPreview,
    decisions: &[SyncDecision],
    keys: &SubKeys,
    actor: Uuid,
) -> Result<VaultData> {
    if revision(local)? != preview.local_revision || revision(remote)? != preview.remote_revision {
        return Err(fail("Vault changed during review; compare again"));
    }
    let checked = self::preview(local, remote, keys)?;
    if decisions
        .iter()
        .any(|d| !checked.changes.iter().any(|c| c.key == d.key))
    {
        return Err(fail("Unknown synchronization decision"));
    }
    let decision_map: BTreeMap<_, _> = decisions.iter().map(|d| (d.key.as_str(), d)).collect();
    if decision_map.len() != decisions.len() {
        return Err(fail("Duplicate synchronization decision"));
    }
    let mut result = local.clone();
    for (id, history) in &remote.settings.sync.history {
        let target = result.settings.sync.history.entry(*id).or_default();
        for version in history {
            if !target.iter().any(|old| old.id == version.id) {
                target.push(version.clone());
            }
        }
        trim_history(target);
    }
    join(&mut result.settings.sync.clock, &remote.settings.sync.clock);
    if !checked.changes.is_empty() {
        tick(&mut result.settings.sync.clock, actor)?;
    }
    let now = Utc::now();
    let remote_fields = all_fields(remote)?;
    let local_fields = all_fields(local)?;
    let mut memberships: BTreeMap<Uuid, BTreeSet<String>> = local.entries.iter()
        .chain(local.trash.iter().map(|t| &t.entry))
        .map(|item| Ok((item.id, tag_memberships(local, item)?)))
        .collect::<Result<_>>()?;
    let mut affected = BTreeSet::new();
    let mut tag_order = None;
    let mut duplicated: BTreeSet<_> = checked
        .changes
        .iter()
        .filter(|c| {
            c.keep_both_entries.is_some()
                && decision_map
                    .get(c.key.as_str())
                    .is_some_and(|d| d.choice == Choice::Both)
        })
        .map(|c| c.entry_id)
        .collect();
    for change in &checked.changes {
        let explicit = decision_map.get(change.key.as_str());
        if duplicated.contains(&change.entry_id) && !explicit.is_some_and(|decision| decision.choice == Choice::Both && change.keep_both_entries.is_some()) {
            if explicit.is_some() {
                return Err(fail(
                    "Keeping both passwords preserves two complete accounts; remove individual field overrides for this entry",
                ));
            }
            continue;
        }
        let choice = explicit
            .map(|d| &d.choice)
            .or(change.suggested.as_ref())
            .ok_or_else(|| fail("Resolve every conflict before synchronizing"))?;
        if *choice == Choice::Both && !change.can_keep_both
            || *choice == Choice::Edited && !change.can_edit
        {
            return Err(fail("Unsupported conflict resolution"));
        }
        let id = change.entry_id;
        affected.insert(id);
        if change.field == "vault_name" {
            let name = match choice {
                Choice::Local => &local.metadata.name,
                Choice::Remote => &remote.metadata.name,
                Choice::Edited => explicit
                    .and_then(|d| d.text.as_ref())
                    .ok_or_else(|| fail("Missing edited value"))?,
                Choice::Both => return Err(fail("Unsupported conflict resolution")),
            };
            crate::vault::validation::validate_display_name(name)?;
            result.metadata.name = name.clone();
        } else if change.field == "tag_order" {
            let source = if *choice == Choice::Remote {
                remote
            } else {
                local
            };
            tag_order = Some(source.tags.iter().map(|t| t.id).collect::<Vec<_>>());
        } else if change.field == "state" {
            let source = if *choice == Choice::Remote {
                remote
            } else {
                local
            };
            result.entries.retain(|e| e.id != id);
            result.trash.retain(|t| t.entry.id != id);
            memberships.insert(id, entry(source, &id).map(|item| tag_memberships(source, item)).transpose()?.unwrap_or_default());
            if let Some(item) = source.entries.iter().find(|e| e.id == id) {
                result.entries.push(item.clone());
            }
            if let Some(item) = source.trash.iter().find(|t| t.entry.id == id) {
                result.trash.push(item.clone());
            }
        } else if change.field == "tag" {
            let source = if *choice == Choice::Remote {
                remote
            } else {
                local
            };
            let tag = if *choice == Choice::Edited {
                let mut tag = local.tags.iter().chain(&remote.tags).find(|tag| tag.id == id).cloned()
                    .ok_or_else(|| fail("Tag disappeared during review"))?;
                let name = explicit.and_then(|decision| decision.text.as_ref()).ok_or_else(|| fail("Missing edited tag name"))?;
                crate::vault::validation::validate_display_name(name)?;
                tag.name = name.clone();
                Some(tag)
            } else { source.tags.iter().find(|t| t.id == id).cloned() };
            result.tags.retain(|t| t.id != id);
            if let Some(tag) = tag {
                result.tags.push(tag);
            }
        } else if is_tag_membership(&change.field) {
            let source = if *choice == Choice::Remote { &remote_fields } else { &local_fields };
            let selected = memberships.entry(id).or_default();
            if source.0.get(&change.key) == Some(&Value::Bool(true)) {
                selected.insert(change.field.clone());
            } else {
                selected.remove(&change.field);
            }
        } else {
            let target = result
                .entries
                .iter_mut()
                .chain(result.trash.iter_mut().map(|t| &mut t.entry))
                .find(|e| e.id == id)
                .ok_or_else(|| fail("Entry disappeared during review"))?;
            match choice {
                Choice::Remote => apply_field(
                    target,
                    &change.field,
                    remote_fields
                        .0
                        .get(&change.key)
                        .unwrap_or(&Value::Null)
                        .clone(),
                )?,
                Choice::Edited => {
                    let text = explicit
                        .and_then(|d| d.text.as_ref())
                        .ok_or_else(|| fail("Missing edited value"))?;
                    if text.len() > 1_048_576 {
                        return Err(fail("Edited field exceeds the supported size"));
                    }
                    if change.field == "title" {
                        crate::vault::validation::validate_display_name(text)?;
                    }
                    apply_field(target, &change.field, Value::String(text.clone()))?;
                }
                Choice::Both if change.field == "notes" => {
                    let other = entry(remote, &id).ok_or_else(|| fail("Missing remote note"))?;
                    target.custom_fields.push(crate::vault::CustomField {
                        id: Uuid::new_v4(),
                        name: "Notes (other device)".into(),
                        field_type: crate::vault::FieldType::Notes,
                        value: other.notes.clone(),
                        sensitive: true,
                    });
                }
                Choice::Both => {
                    duplicated.insert(id);
                }
                Choice::Local => {}
            }
            target.updated_at = now;
            if change.field == "encrypted_password" {
                target.password_changed_at = now;
                target.breach_status = Default::default();
                target.strength_score = None;
            }
        }
    }
    if let Some(order) = tag_order {
        result.tags.sort_by_key(|tag| {
            order
                .iter()
                .position(|id| *id == tag.id)
                .unwrap_or(usize::MAX)
        });
    }
    for id in duplicated {
        let source = entry(remote, &id).ok_or_else(|| fail("Missing remote entry"))?;
        let new_id = copy_identity(preview.id, id);
        if entry(&result, &new_id).is_some() {
            return Err(fail("Duplicate result identity; compare again"));
        }
        let mut copy = duplicate(source, keys, new_id)?;
        memberships.insert(new_id, tag_memberships(remote, source)?);
        copy.title = result_title(&copy.title, " (other device)");
        if let Some(original) = result
            .entries
            .iter_mut()
            .chain(result.trash.iter_mut().map(|t| &mut t.entry))
            .find(|e| e.id == id)
        {
            original.title = result_title(&original.title, " (this device)");
        }
        result.entries.push(copy);
    }
    let mut names = BTreeSet::new();
    if result.tags.iter().any(|tag| !names.insert(tag.name.as_str())) {
        return Err(fail("Tag names overlap; edit one tag name in the review or remove one of the tags"));
    }
    let projected: BTreeMap<_, Vec<_>> = memberships.into_iter().map(|(id, fields)| {
        (id, fields.iter().filter_map(|field| membership_name(&result, field)).collect())
    }).collect();
    for item in result.entries.iter_mut().chain(result.trash.iter_mut().map(|t| &mut t.entry)) {
        if let Some(names) = projected.get(&item.id) { item.tags = names.clone(); }
    }
    // Join equal values too, so a later edit supersedes both observed versions.
    for (field_key, revision) in &remote.settings.sync.fields {
        let (id, field) = parts(field_key)?;
        if checked
            .changes
            .iter()
            .any(|c| c.key == *field_key || c.entry_id == id && c.field == "state")
        {
            continue;
        }
        if field == "state" && state(local, id) == "deleted" && state(remote, id) == "deleted"
            || fingerprint(
                local_fields.0.get(field_key).unwrap_or(&Value::Null),
                keys,
                id,
                field,
            )? == fingerprint(
                remote_fields.0.get(field_key).unwrap_or(&Value::Null),
                keys,
                id,
                field,
            )?
        {
            if let Some(existing) = result.settings.sync.fields.get_mut(field_key) {
                join(&mut existing.clock, &revision.clock);
            } else {
                result
                    .settings
                    .sync
                    .fields
                    .insert(field_key.clone(), revision.clone());
            }
        }
    }
    let final_fields = all_fields(&result)?;
    let field_keys: BTreeSet<_> = local
        .settings
        .sync
        .fields
        .keys()
        .chain(remote.settings.sync.fields.keys())
        .chain(final_fields.0.keys())
        .cloned()
        .collect();
    for field_key in field_keys {
        let (id, field) = parts(&field_key)?;
        if checked
            .changes
            .iter()
            .any(|c| c.key == field_key || c.entry_id == id && c.field == "state")
        {
            let fingerprint = fingerprint(
                final_fields.0.get(&field_key).unwrap_or(&Value::Null),
                keys,
                id,
                field,
            )?;
            let change = checked
                .changes
                .iter()
                .find(|c| c.key == field_key || c.entry_id == id && c.field == "state");
            let inherited = change.filter(|c| !c.conflict).and_then(|c| {
                let choice = decision_map
                    .get(c.key.as_str())
                    .map(|d| &d.choice)
                    .or(c.suggested.as_ref());
                if choice != c.suggested.as_ref() {
                    return None;
                }
                match choice {
                    Some(Choice::Local) => local.settings.sync.fields.get(&field_key),
                    Some(Choice::Remote) => remote.settings.sync.fields.get(&field_key),
                    _ => None,
                }
            });
            let revision = inherited.cloned().unwrap_or_else(|| Revision {
                clock: result.settings.sync.clock.clone(),
                fingerprint,
                device: super::resolve_local_device_info(None).id,
                changed_at: now,
                time_reliable: true,
            });
            result.settings.sync.fields.insert(field_key, revision);
        }
    }
    for id in affected {
        if state(&result, id) == "deleted" {
            result.settings.sync.history.remove(&id);
            continue;
        }
        if let Some(target) = result
            .entries
            .iter_mut()
            .chain(result.trash.iter_mut().map(|t| &mut t.entry))
            .find(|e| e.id == id)
        {
            merge_password_history(target, [entry(local, &id), entry(remote, &id)], keys)?;
        }
        let history = result.settings.sync.history.entry(id).or_default();
        for source in [entry(local, &id), entry(remote, &id)]
            .into_iter()
            .flatten()
        {
            let bytes = Zeroizing::new(
                rmp_serde::to_vec(source).map_err(|_| fail("Cannot preserve conflict history"))?,
            );
            history.push(ConflictVersion {
                id: Uuid::new_v4(),
                saved_at: now,
                clock: result.settings.sync.clock.clone(),
                encrypted_entry: VaultManager::encrypt_entry_field(
                    &bytes,
                    &keys.entry_key,
                    &id,
                    "sync-history",
                )?,
            });
        }
        trim_history(history);
    }
    track(&mut result, keys, actor, now)?;
    let retained: BTreeSet<_> = result
        .entries
        .iter()
        .chain(result.trash.iter().map(|item| &item.entry))
        .map(|item| item.id)
        .collect();
    result
        .settings
        .sync
        .history
        .retain(|id, _| retained.contains(id));
    if !checked.changes.is_empty() {
        result.settings.sync.receipts.push(preview.id);
    }
    if result.settings.sync.receipts.len() > 32 {
        result.settings.sync.receipts.remove(0);
    }
    result.metadata.entry_count = result.entries.len();
    Ok(result)
}

fn merge_password_history(
    target: &mut Entry,
    sources: [Option<&Entry>; 2],
    keys: &SubKeys,
) -> Result<()> {
    let current = VaultManager::decrypt_entry_field(
        &target.encrypted_password,
        &keys.entry_key,
        &target.id,
        FieldScope::Password,
    )?;
    let hash_key = Zeroizing::new(blake3::derive_key(
        "yntra-sync-history-comparison-v1",
        &keys.hmac_key.bytes,
    ));
    let mut seen = BTreeSet::from([*blake3::keyed_hash(&hash_key, &current).as_bytes()]);
    let mut history = Vec::new();
    for source in sources.into_iter().flatten() {
        for (blob, scope, time) in source
            .password_history
            .iter()
            .map(|h| (&h.encrypted_password, FieldScope::History, h.changed_at))
            .chain(std::iter::once((
                &source.encrypted_password,
                FieldScope::Password,
                source.password_changed_at,
            )))
        {
            let bytes = VaultManager::decrypt_entry_field(blob, &keys.entry_key, &source.id, scope)
                .or_else(|_| {
                    VaultManager::decrypt_entry_field(
                        blob,
                        &keys.entry_key,
                        &source.id,
                        FieldScope::Password,
                    )
                })?;
            if seen.insert(*blake3::keyed_hash(&hash_key, &bytes).as_bytes()) {
                history.push(crate::vault::PasswordHistoryItem {
                    encrypted_password: VaultManager::encrypt_entry_field(
                        &bytes,
                        &keys.entry_key,
                        &target.id,
                        FieldScope::History,
                    )?,
                    changed_at: time,
                });
            }
        }
    }
    history.sort_by_key(|h| h.changed_at);
    if history.len() > crate::vault::MAX_PASSWORD_HISTORY {
        history.drain(..history.len() - crate::vault::MAX_PASSWORD_HISTORY);
    }
    target.password_history = history;
    Ok(())
}
