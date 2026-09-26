//! Yntra Vault — SOTA Command Line Interface
//!
//! Features IPC session daemon (<5ms latency), `yntra run` environment secret injection,
//! Ratatui interactive TUI, defended clipboards, zero-disclosure search, HIBP breach check,
//! Git Credential Helper protocol, built-in SSH Agent socket, Browser Extension Native Host,
//! Shamir secret recovery, WebDAV sync, and auto-generated shell completions.

use std::path::{Path, PathBuf};
use std::env;
use std::io::Write;
use std::time::Duration;
use clap::{Parser, Subcommand, Args, CommandFactory};
use clap_complete::{generate, Shell};
use colored::*;
use comfy_table::{Table, Row, Cell, Attribute, Color};
use rpassword::prompt_password;
use zeroize::Zeroizing;
use uuid::Uuid;
use sha2::{Sha256, Digest};
use serde::{Deserialize, Serialize};

use yntra_vault_core::{
    VaultError, Result,
    vault::{
        VaultManager, EntryPreview,
        types::SecurityAudit,
        manager::{NewEntry, UpdateEntry, DecryptedEntry},
        importer::{Importer, ImportFormat, DuplicateStrategy},
    },
    services::{
        autotype::run_smart_autotype,
        sync::{
            webdav_upload, webdav_download, run_p2p_sync_listener, run_p2p_sync_client,
            generate_pairing_code, run_p2p_pairing_host, run_p2p_pairing_client,
            compute_pairing_beacon_id, listen_pairing_beacon,
        },
    },
    totp::{generate_totp, parse_otpauth_uri, TotpConfig},
    generator::{generate_password, GeneratorOptions, GeneratorMode},
    breach::{check_password_breach, strength::analyze_password},
    crypto::{
        clipboard::copy_to_clipboard_defended,
        sharing::{split_secret, reconstruct_secret},
        biometric::check_biometric_availability,
    },
};

mod ipc;
mod run_cmd;
mod tui;
mod keychain;
mod git_credential;
mod ssh_agent;
mod native_host;

use ipc::{try_ipc_request, run_ipc_daemon, IpcRequest, IpcResponse};
use run_cmd::execute_secret_injection;
use tui::run_tui;
use git_credential::handle_git_credential;
use ssh_agent::run_ssh_agent_pipe;
use native_host::{run_native_host_loop, install_native_host_manifest, is_browser_native_host_invocation};
use keychain::clear_session_token;

#[derive(Parser)]
#[command(
    name = "yntra",
    author = "Yntra Vault Team",
    version = env!("CARGO_PKG_VERSION"),
    about = "Yntra Vault — High-Security Offline-First Password Manager CLI",
    long_about = None
)]
struct Cli {
    /// Path to .vdb vault database file [env: YNTRA_VAULT_PATH]
    #[arg(short = 'v', long, global = true, env = "YNTRA_VAULT_PATH")]
    path: Option<PathBuf>,

    /// Master password for non-interactive execution [env: YNTRA_PASSWORD]
    #[arg(short = 'p', long, global = true, env = "YNTRA_PASSWORD", hide = true)]
    password: Option<String>,

    /// Keyfile path for dual-factor vault unlock [env: YNTRA_KEYFILE]
    #[arg(short = 'k', long, global = true, env = "YNTRA_KEYFILE")]
    keyfile: Option<PathBuf>,

    /// Output results in raw JSON format for scripting
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Unlock session and start background IPC daemon (<5ms execution)
    Unlock,
    /// Lock active session and terminate background IPC daemon
    Lock,
    /// Inject vault secrets into child process environment (yntra run -- <cmd>)
    Run(RunArgs),
    /// Launch interactive Terminal User Interface (TUI)
    Tui,
    /// Official Git Credential Helper protocol handler
    GitCredential(GitCredentialArgs),
    /// Start in-memory SSH Agent socket (SSH_AUTH_SOCK)
    SshAgent,
    /// Browser Extension Native Messaging Host protocol
    NativeHost(NativeHostArgs),
    /// Generate shell auto-completion scripts for Zsh, Bash, Fish, or PowerShell
    Completions(CompletionsArgs),
    /// Initialize a new encrypted vault
    Init(InitArgs),
    /// Display vault header metadata and encryption configuration
    Info,
    /// List entries in the vault
    List(ListArgs),
    /// Retrieve entry details by ID or title
    Get(GetArgs),
    /// Add a new entry to the vault
    Add(AddArgs),
    /// Edit an existing entry in the vault
    Edit(EditArgs),
    /// Delete an entry from the vault
    Delete(DeleteArgs),
    /// Generate a live TOTP 6-digit verification code
    Totp(TotpArgs),
    /// Cryptographically secure password & diceware generator
    Generate(GenerateArgs),
    /// Fast zero-disclosure trigram search across entries
    Search(SearchArgs),
    /// Audit vault security for weak, old, reused, or breached passwords
    Audit,
    /// Check a password against public breach databases using HIBP k-anonymity
    CheckBreach(CheckBreachArgs),
    /// Import credentials from Bitwarden JSON, CSV, 1Password, or KeePass
    Import(ImportArgs),
    /// Export vault contents to CSV, JSON, or encrypted .vdb file
    Export(ExportArgs),
    /// Shamir 2-of-3 secret sharing recovery tool
    Shamir(ShamirArgs),
    /// Generate or use recovery v2 for this vault copy
    Recovery(RecoveryArgs),
    /// Synchronize vault database with WebDAV remote or P2P peer
    Sync(SyncArgs),
    /// Trigger OS-level smart autotype into focused active application
    Autotype(AutotypeArgs),
    /// Change master password or keyfile binding
    ChangePassword(ChangePasswordArgs),
    /// Manage biometric unlock options
    Biometric(BiometricArgs),
    /// Check for updates and upgrade yntra CLI in-place
    Update(UpdateArgs),
}

#[derive(Args)]
struct RecoveryArgs { #[command(subcommand)] action: RecoveryAction }
#[derive(Subcommand)]
enum RecoveryAction {
    /// Save three separate recovery share files; keep them in separate safe places
    Generate { #[arg(long)] output_dir: PathBuf },
    /// Recover access from two saved share files and choose a new password
    Restore { #[arg(long)] share_a_file: PathBuf, #[arg(long)] share_b_file: PathBuf },
    /// Revoke the current kit (USB-bound vaults require a replacement kit instead)
    Revoke,
}

#[derive(Args)]
struct UpdateArgs {
    /// Only check if an update is available without installing
    #[arg(short, long)]
    check: bool,
    /// Automatically proceed with installation without interactive prompt
    #[arg(short = 'y', long)]
    yes: bool,
    /// Custom update manifest endpoint URL
    #[arg(long, hide = true)]
    endpoint: Option<String>,
}

#[derive(Args)]
struct GitCredentialArgs {
    /// Action: get, store, erase, setup
    action: String,
}

#[derive(Args)]
struct NativeHostArgs {
    /// Subcommand action: run (stdio host mode) or install (manifest installer)
    #[arg(default_value = "run")]
    action: String,
    /// Target browser for manifest installation: chrome, firefox, edge
    #[arg(default_value = "chrome")]
    browser: String,
}

#[derive(Args)]
struct RunArgs {
    /// Optional path to .env or .env.tpl template file
    #[arg(short = 'e', long)]
    env_file: Option<PathBuf>,
    /// Target command to execute
    cmd: String,
    /// Additional arguments to pass to child command
    args: Vec<String>,
}

#[derive(Args)]
struct CompletionsArgs {
    /// Target shell: bash, zsh, fish, powershell, elvish
    shell: Shell,
}

#[derive(Args)]
struct InitArgs {
    /// Name for the new vault metadata
    #[arg(short, long, default_value = "Main Vault")]
    name: String,
    /// Automatically generate and bind a new 32-byte cryptographic keyfile at the specified path
    #[arg(long, value_name = "PATH")]
    gen_keyfile: Option<PathBuf>,
}

#[derive(Args)]
struct ListArgs {
    /// Filter by specific tag
    #[arg(short, long)]
    tag: Option<String>,
    /// Filter only favorite entries
    #[arg(short, long)]
    favorite: bool,
    /// Filter by search query
    #[arg(short, long)]
    search: Option<String>,
}

#[derive(Args)]
struct GetArgs {
    /// Entry title or UUID
    query: String,
    /// Print decrypted password to stdout
    #[arg(short = 's', long)]
    show_password: bool,
    /// Copy password to OS defended clipboard
    #[arg(short = 'c', long)]
    copy: bool,
    /// Generate current TOTP code for entry
    #[arg(long)]
    totp: bool,
}

#[derive(Args)]
struct AddArgs {
    /// Entry title
    #[arg(short = 't', long)]
    title: String,
    /// Username / Login handle
    #[arg(short = 'u', long)]
    username: Option<String>,
    /// Password value (leave empty to prompt or autogenerate)
    #[arg(short = 'e', long)]
    entry_password: Option<String>,
    /// Auto-generate a secure 24-character password
    #[arg(short = 'g', long)]
    generate: bool,
    /// Website URL
    #[arg(short = 'l', long)]
    url: Option<String>,
    /// Account Email
    #[arg(long)]
    email: Option<String>,
    /// Secure notes
    #[arg(short = 'n', long)]
    notes: Option<String>,
    /// Comma-separated list of tags
    #[arg(long)]
    tags: Option<String>,
    /// TOTP Secret Key (Base32 or otpauth:// URI)
    #[arg(long)]
    totp_secret: Option<String>,
}

#[derive(Args)]
struct EditArgs {
    /// Entry title or UUID to edit
    query: String,
    /// New entry title
    #[arg(short = 't', long)]
    title: Option<String>,
    /// New username
    #[arg(short = 'u', long)]
    username: Option<String>,
    /// New password value
    #[arg(short = 'e', long)]
    entry_password: Option<String>,
    /// New URL
    #[arg(short = 'l', long)]
    url: Option<String>,
    /// New Email
    #[arg(long)]
    email: Option<String>,
    /// New notes
    #[arg(short = 'n', long)]
    notes: Option<String>,
    /// New comma-separated tags
    #[arg(long)]
    tags: Option<String>,
    /// New TOTP Secret
    #[arg(long)]
    totp_secret: Option<String>,
}

#[derive(Args)]
struct DeleteArgs {
    /// Entry title or UUID to delete
    query: String,
    /// Permanently remove entry instead of moving to trash
    #[arg(long)]
    permanent: bool,
}

#[derive(Args)]
struct TotpArgs {
    /// Entry title, UUID, or raw Base32 secret
    query: String,
    /// Copy TOTP code to defended clipboard
    #[arg(short, long)]
    copy: bool,
}

#[derive(Args)]
struct GenerateArgs {
    /// Generate a cryptographically secure 32-byte raw keyfile at specified path
    #[arg(long, value_name = "PATH")]
    keyfile: Option<PathBuf>,
    /// Password length (character count)
    #[arg(short, long, default_value = "24")]
    length: usize,
    /// Use diceware word-based passphrase generation
    #[arg(short, long)]
    diceware: bool,
    /// Number of words for diceware passphrase
    #[arg(short, long, default_value = "5")]
    words: usize,
    /// Exclude uppercase letters
    #[arg(long)]
    no_uppercase: bool,
    /// Exclude lowercase letters
    #[arg(long)]
    no_lowercase: bool,
    /// Exclude numeric digits
    #[arg(long)]
    no_numbers: bool,
    /// Exclude special symbols
    #[arg(long)]
    no_symbols: bool,
    /// Copy generated password to defended clipboard
    #[arg(short, long)]
    copy: bool,
}

#[derive(Args)]
struct SearchArgs {
    /// Search term
    query: String,
}

#[derive(Args)]
struct CheckBreachArgs {
    /// Password to check (prompts securely if omitted)
    password: Option<String>,
}

#[derive(Args)]
struct ImportArgs {
    /// Source file path to import
    #[arg(short = 'i', long)]
    file: PathBuf,
    /// Import format: bitwarden, csv, 1password, keepass
    #[arg(short = 'f', long)]
    format: Option<String>,
    /// Strategy for duplicate entries: skip, overwrite, keep-both
    #[arg(long, default_value = "skip")]
    on_duplicate: String,
}

#[derive(Args)]
struct ExportArgs {
    /// Destination output file path
    #[arg(short, long)]
    dest: PathBuf,
    /// Export format: vdb, csv, json
    #[arg(short, long, default_value = "csv")]
    format: String,
    /// Force unencrypted plain-text export without confirmation prompt
    #[arg(long)]
    force: bool,
}

#[derive(Args)]
struct ShamirArgs {
    #[command(subcommand)]
    action: ShamirAction,
}

#[derive(Subcommand)]
enum ShamirAction {
    /// Split master password into 3 Shamir shares (2 required)
    Split {
        /// Password to split (prompts securely if omitted)
        #[arg(short, long)]
        password: Option<String>,
    },
    /// Reconstruct master password hash from 2 shares
    Recover {
        /// First share key (YNTRA-SHARE1-... or legacy SL-SHARE1-...)
        #[arg(short = 'a', long)]
        share_a: String,
        /// Second share key (YNTRA-SHARE2-... or legacy SL-SHARE2-...)
        #[arg(short = 'b', long)]
        share_b: String,
    },
}

#[derive(Args)]
struct SyncArgs {
    #[command(subcommand)]
    action: SyncAction,
}

#[derive(Subcommand)]
enum SyncAction {
    /// Push vault database to remote WebDAV server
    WebdavUpload {
        /// Remote WebDAV file endpoint URL
        #[arg(short, long)]
        url: String,
        /// WebDAV HTTP Auth Username
        #[arg(short, long)]
        username: String,
        /// WebDAV HTTP Auth Password (prompts if omitted)
        #[arg(short = 'e', long)]
        password: Option<String>,
    },
    /// Pull vault database from remote WebDAV server
    WebdavDownload {
        /// Remote WebDAV file endpoint URL
        #[arg(short, long)]
        url: String,
        /// WebDAV HTTP Auth Username
        #[arg(short, long)]
        username: String,
        /// WebDAV HTTP Auth Password (prompts if omitted)
        #[arg(short = 'e', long)]
        password: Option<String>,
    },
    /// Run P2P synchronization listener (server mode)
    P2pListen {
        /// Address to listen on (e.g. 0.0.0.0:5322)
        #[arg(short, long, default_value = "0.0.0.0:5322")]
        listen: String,
    },
    /// Connect to remote P2P synchronization peer (client mode)
    P2pConnect {
        /// Target server address (e.g. 192.168.1.10:5322)
        #[arg(short, long)]
        server: String,
    },
    /// Host Zero-Knowledge device pairing session (displays/accepts pairing PIN)
    PairHost {
        /// Address to listen on (e.g. 0.0.0.0:5322)
        #[arg(short, long, default_value = "0.0.0.0:5322")]
        listen: String,
        /// 6-digit pairing PIN code (auto-generated if omitted)
        #[arg(short, long)]
        code: Option<String>,
    },
    /// Connect to remote device using pairing PIN (scans Wi-Fi or connects to server)
    PairConnect {
        /// 6-digit pairing PIN code
        #[arg(short, long)]
        code: String,
        /// Explicit host server address (scans Wi-Fi beacon if omitted)
        #[arg(short, long)]
        server: Option<String>,
    },
}

#[derive(Args)]
struct AutotypeArgs {
    /// Entry title or UUID to autotype
    query: String,
}

#[derive(Args)]
struct ChangePasswordArgs {
    /// New master password (prompts securely if omitted)
    #[arg(long)]
    new_password: Option<String>,
    /// Path to new keyfile to bind
    #[arg(long)]
    new_keyfile: Option<PathBuf>,
}

#[derive(Args)]
struct BiometricArgs {
    #[command(subcommand)]
    action: BiometricAction,
}

#[derive(Subcommand)]
enum BiometricAction {
    /// Enable biometric unlock envelope in vault header
    Enable,
    /// Disable biometric unlock
    Disable,
    /// Check biometric hardware availability
    Status,
}

#[tokio::main]
async fn main() {
    let raw_args: Vec<String> = std::env::args().collect();
    if is_browser_native_host_invocation(&raw_args) {
        if let Err(err) = run_native_host_loop().await {
            eprintln!("{} {}", "Native Host Error:".red().bold(), err);
            std::process::exit(1);
        }
        return;
    }

    let cli = Cli::parse();

    if let Err(err) = run(cli).await {
        eprintln!("{} {}", "Error:".red().bold(), err);
        let code = get_exit_code(&err);
        std::process::exit(code);
    }
}

fn get_exit_code(err: &VaultError) -> i32 {
    match err {
        VaultError::VaultLocked => 2,
        VaultError::VaultNotFound(_) | VaultError::EntryNotFound(_) => 3,
        VaultError::InvalidFormat(_) | VaultError::VaultAlreadyExists(_) => 4,
        VaultError::NetworkError(_) | VaultError::SyncError(_) => 5,
        _ => 1,
    }
}

async fn run(cli: Cli) -> Result<()> {
    if std::env::args().any(|a| a == "-p" || a.starts_with("--password=") || a == "--password") {
        eprintln!(
            "{}",
            "Security Advisory: Passing master passwords via CLI arguments exposes secrets in the OS process table (ps aux) and shell history. Prefer YNTRA_PASSWORD env var, stdin pipe, or 'yntra unlock' session daemon."
                .yellow()
        );
    }

    let vault_path = cli.path.unwrap_or_else(|| {
        PathBuf::from(env::var("YNTRA_VAULT_PATH").unwrap_or_else(|_| "vault.vdb".to_string()))
    });

    let command = match cli.command {
        Some(cmd) => cmd,
        None => {
            use std::io::IsTerminal;
            if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
                Commands::Tui
            } else {
                use clap::CommandFactory;
                let _ = Cli::command().print_help();
                println!();
                return Ok(());
            }
        }
    };

    cleanup_old_binary_if_present();

    if !matches!(command, Commands::Update(_) | Commands::Completions(_) | Commands::NativeHost(_) | Commands::GitCredential(_)) {
        maybe_print_update_tip().await;
    }

    match command {
        Commands::Unlock => handle_unlock(&vault_path, cli.password, cli.keyfile).await,
        Commands::Lock => handle_lock().await,
        Commands::Run(args) => execute_secret_injection(&vault_path, cli.password, cli.keyfile.as_deref(), args.env_file, args.cmd, args.args).await,
        Commands::Tui => run_tui(&vault_path, cli.password, cli.keyfile.as_deref()).await,
        Commands::GitCredential(args) => handle_git_credential(&args.action).await,
        Commands::SshAgent => run_ssh_agent_pipe().await,
        Commands::NativeHost(args) => handle_native_host(&args).await,
        Commands::Completions(args) => handle_completions(args.shell),
        Commands::Init(args) => handle_init(&vault_path, &args, cli.password, cli.keyfile.as_deref()),
        Commands::Info => handle_info(&vault_path, cli.json),
        Commands::List(args) => handle_list(&vault_path, &args, cli.password, cli.keyfile.as_deref(), cli.json).await,
        Commands::Get(args) => handle_get(&vault_path, &args, cli.password, cli.keyfile.as_deref(), cli.json).await,
        Commands::Add(args) => handle_add(&vault_path, &args, cli.password, cli.keyfile.as_deref()).await,
        Commands::Edit(args) => handle_edit(&vault_path, &args, cli.password, cli.keyfile.as_deref()).await,
        Commands::Delete(args) => handle_delete(&vault_path, &args, cli.password, cli.keyfile.as_deref()).await,
        Commands::Totp(args) => handle_totp(&vault_path, &args, cli.password, cli.keyfile.as_deref()).await,
        Commands::Generate(args) => handle_generate(&args),
        Commands::Search(args) => handle_search(&vault_path, &args, cli.password, cli.keyfile.as_deref(), cli.json).await,
        Commands::Audit => handle_audit(&vault_path, cli.password, cli.keyfile.as_deref(), cli.json).await,
        Commands::CheckBreach(args) => handle_check_breach(&args).await,
        Commands::Import(args) => handle_import(&vault_path, &args, cli.password, cli.keyfile.as_deref()),
        Commands::Export(args) => handle_export(&vault_path, &args, cli.password, cli.keyfile.as_deref()),
        Commands::Shamir(args) => handle_shamir(&args),
        Commands::Recovery(args) => handle_recovery(&vault_path, &args, cli.password.clone(), cli.keyfile.as_deref()),
        Commands::Sync(args) => handle_sync(&vault_path, &args, cli.password, cli.keyfile.as_deref()).await,
        Commands::Autotype(args) => handle_autotype(&vault_path, &args, cli.password, cli.keyfile.as_deref()).await,
        Commands::ChangePassword(args) => handle_change_password(&vault_path, &args, cli.password, cli.keyfile.as_deref()),
        Commands::Biometric(args) => handle_biometric(&vault_path, &args, cli.password, cli.keyfile.as_deref()),
        Commands::Update(args) => handle_update(args, cli.json).await,
    }
}

async fn handle_native_host(args: &NativeHostArgs) -> Result<()> {
    match args.action.to_lowercase().as_str() {
        "install" => install_native_host_manifest(&args.browser),
        "run" | "stdio" => run_native_host_loop().await,
        other => Err(VaultError::InvalidFormat(format!("Unknown native-host action '{}'. Allowed: run, install", other))),
    }
}

async fn handle_unlock(vault_path: &Path, password: Option<String>, keyfile: Option<PathBuf>) -> Result<()> {
    if try_ipc_request(&IpcRequest::Ping).await.is_some() {
        println!("{} Session IPC daemon is already active!", "✓".green().bold());
        return Ok(());
    }
    let pass = acquire_password(password)?;
    run_ipc_daemon(vault_path.to_path_buf(), pass, keyfile).await
}

async fn handle_lock() -> Result<()> {
    let _ = clear_session_token();
    if let Some(IpcResponse::LockSuccess) = try_ipc_request(&IpcRequest::Lock).await {
        println!("{} Vault session locked. Key material zeroized.", "🔒".yellow().bold());
    } else {
        println!("No active IPC session daemon found.");
    }
    Ok(())
}

fn handle_completions(shell: Shell) -> Result<()> {
    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "yntra", &mut std::io::stdout());
    Ok(())
}

/// Helper to obtain master password interactively or from arguments/environment
fn acquire_password(provided: Option<String>) -> Result<Zeroizing<String>> {
    if let Some(pass) = provided {
        eprintln!(
            "{} Security Warning: Passing passwords via CLI flags exposes them in system process lists. Use YNTRA_PASSWORD env var or interactive prompts.",
            "⚠️".yellow().bold()
        );
        return Ok(Zeroizing::new(pass));
    }
    if let Ok(env_pass) = env::var("YNTRA_PASSWORD")
        && !env_pass.is_empty() {
            return Ok(Zeroizing::new(env_pass));
        }
    let input = prompt_password("Enter Master Password: ")
        .map_err(|e| VaultError::InvalidFormat(format!("Terminal input error: {}", e)))?;
    if input.is_empty() {
        return Err(VaultError::InvalidFormat("Master password cannot be empty".into()));
    }
    Ok(Zeroizing::new(input))
}

pub fn open_vault(vault_path: &Path, password: Option<String>, keyfile: Option<&Path>) -> Result<VaultManager> {
    if !vault_path.exists() {
        return Err(VaultError::VaultNotFound(format!("Vault database file not found at: {}", vault_path.display())));
    }
    let pass = acquire_password(password)?;
    VaultManager::open_with_keyfile(vault_path, &pass, keyfile)
}

pub fn resolve_entry_id(manager: &VaultManager, query: &str) -> Result<Uuid> {
    if let Ok(id) = Uuid::parse_str(query) {
        return Ok(id);
    }
    let entries = manager.list_entries()?;
    let matches: Vec<_> = entries.iter().filter(|e| e.title.to_lowercase().contains(&query.to_lowercase())).collect();
    if matches.is_empty() {
        return Err(VaultError::EntryNotFound(format!("No entry matching title/UUID '{}'", query)));
    }
    let exact_matches: Vec<_> = matches.iter().filter(|e| e.title.eq_ignore_ascii_case(query)).collect();
    if exact_matches.len() == 1 {
        return Ok(exact_matches[0].id);
    }
    if exact_matches.len() > 1 {
        return Err(VaultError::InvalidFormat(format!(
            "Ambiguous match: Found {} entries titled '{}'. Please specify exact UUID.",
            exact_matches.len(), query
        )));
    }
    if matches.len() > 1 {
        return Err(VaultError::InvalidFormat(format!(
            "Ambiguous match for '{}': matches {} entries. Please specify exact UUID or full title.",
            query, matches.len()
        )));
    }
    Ok(matches[0].id)
}

fn build_totp_config(secret_or_uri: &str) -> Result<TotpConfig> {
    if secret_or_uri.starts_with("otpauth://") {
        parse_otpauth_uri(secret_or_uri)
    } else {
        Ok(TotpConfig {
            secret: secret_or_uri.to_string(),
            ..Default::default()
        })
    }
}

// ─── Subcommand Handlers ───────────────────────────────────────────────────

fn handle_init(vault_path: &Path, args: &InitArgs, password: Option<String>, keyfile: Option<&Path>) -> Result<()> {
    if vault_path.exists() {
        return Err(VaultError::VaultAlreadyExists(format!("Vault file already exists at: {}", vault_path.display())));
    }
    
    // Determine effective keyfile path: explicit keyfile or newly generated keyfile
    let effective_keyfile = if let Some(gen_path) = &args.gen_keyfile {
        VaultManager::generate_key_file(gen_path)?;
        println!("{} Generated 32-byte keyfile at {}", "✓".green().bold(), gen_path.display().to_string().yellow());
        Some(gen_path.as_path())
    } else {
        keyfile
    };

    println!("{} Creating new vault at {}", "→".blue().bold(), vault_path.display().to_string().yellow());
    let pass = acquire_password(password.clone())?;
    
    if password.is_none() && env::var("YNTRA_PASSWORD").is_err() {
        let confirm = prompt_password("Confirm Master Password: ")
            .map_err(|e| VaultError::InvalidFormat(format!("Terminal input error: {}", e)))?;
        if *pass != confirm {
            return Err(VaultError::InvalidFormat("Passwords do not match!".into()));
        }
    }

    let _manager = VaultManager::create_with_keyfile(&args.name, &pass, effective_keyfile, vault_path)?;
    println!("{} Vault created successfully!", "✓".green().bold());
    Ok(())
}

fn handle_info(vault_path: &Path, json: bool) -> Result<()> {
    if !vault_path.exists() {
        return Err(VaultError::VaultNotFound(format!("Vault file not found at: {}", vault_path.display())));
    }
    let file_bytes = std::fs::read(vault_path)
        .map_err(|e| VaultError::VaultNotFound(format!("{}: {}", vault_path.display(), e)))?;
    if yntra_vault_core::vault::storage::is_protected(&file_bytes) {
        let (header, payload) = yntra_vault_core::vault::storage::parse(&file_bytes)?;
        let info=serde_json::json!({"path":vault_path.to_string_lossy(),"format":"YNS2","version":header.version,"usb_bound":header.usb_marker.is_some(),"recovery_enabled":header.recovery.is_some(),"payload_bytes":payload.ciphertext.len()});
        if json { println!("{}",serde_json::to_string_pretty(&info).map_err(|e|VaultError::SerializationError(e.to_string()))?); }
        else { println!("Local protection v2\nUSB binding: {}\nRecovery: {}\nHeader information is unverified until unlock.",header.usb_marker.is_some(),header.recovery.is_some()); }
        return Ok(());
    }
    let vault_file = yntra_vault_core::vault::format::VaultFile::from_bytes(&file_bytes)?;

    let bio_status = vault_file.biometric.is_some();
    let hw2fa_status = vault_file.hardware2fa.is_some();

    if json {
        let info_obj = serde_json::json!({
            "path": vault_path.to_string_lossy(),
            "version": vault_file.header.version,
            "flags": vault_file.header.flags,
            "biometric_enabled": bio_status,
            "hardware2fa_enabled": hw2fa_status,
            "payload_bytes": vault_file.encrypted_payload.len(),
        });
        println!("{}", serde_json::to_string_pretty(&info_obj).unwrap());
    } else {
        println!("{}", "═════════════════════════════════════════════".blue());
        println!("  {}", "YNTRA VAULT INFORMATION".bold());
        println!("{}", "═════════════════════════════════════════════".blue());
        println!("  {:18} {}", "Database Path:".bold(), vault_path.display());
        println!("  {:18} v{}", "Format Version:".bold(), vault_file.header.version);
        println!("  {:18} {} bytes", "Payload Size:".bold(), vault_file.encrypted_payload.len());
        println!("  {:18} {}", "Biometric Lock:".bold(), if bio_status { "Enabled".green().bold() } else { "Disabled".dimmed() });
        println!("  {:18} {}", "Hardware 2FA:".bold(), if hw2fa_status { "Enabled".green().bold() } else { "Disabled".dimmed() });
        println!("{}", "═════════════════════════════════════════════".blue());
    }
    Ok(())
}

async fn handle_list(
    vault_path: &Path,
    args: &ListArgs,
    password: Option<String>,
    keyfile: Option<&Path>,
    json: bool,
) -> Result<()> {
    let mut entries = if let Some(IpcResponse::ListEntries(list)) = try_ipc_request(&IpcRequest::ListEntries).await {
        list
    } else {
        let manager = open_vault(vault_path, password, keyfile)?;
        manager.list_entries()?
    };

    if let Some(ref tag) = args.tag {
        entries.retain(|e| e.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)));
    }
    if args.favorite {
        entries.retain(|e| e.favorite);
    }
    if let Some(ref search_query) = args.search {
        entries.retain(|e| {
            e.title.to_lowercase().contains(&search_query.to_lowercase())
                || e.username.to_lowercase().contains(&search_query.to_lowercase())
                || e.url.to_lowercase().contains(&search_query.to_lowercase())
        });
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&entries).unwrap());
        return Ok(());
    }

    if entries.is_empty() {
        println!("{}", "No matching vault entries found.".yellow());
        return Ok(());
    }

    let mut table = Table::new();
    table.set_header(vec![
        Cell::new("UUID").add_attribute(Attribute::Bold).fg(Color::Cyan),
        Cell::new("Title").add_attribute(Attribute::Bold).fg(Color::Cyan),
        Cell::new("Username").add_attribute(Attribute::Bold).fg(Color::Cyan),
        Cell::new("URL").add_attribute(Attribute::Bold).fg(Color::Cyan),
        Cell::new("Tags").add_attribute(Attribute::Bold).fg(Color::Cyan),
        Cell::new("2FA").add_attribute(Attribute::Bold).fg(Color::Cyan),
    ]);

    for entry in entries {
        let short_id = entry.id.to_string()[..8].to_string();
        let title_disp = if entry.favorite { format!("★ {}", entry.title) } else { entry.title };
        let tags_disp = entry.tags.join(", ");
        let totp_disp = if entry.has_totp { "TOTP".green().to_string() } else { "-".dimmed().to_string() };

        table.add_row(Row::from(vec![
            Cell::new(short_id).fg(Color::DarkGrey),
            Cell::new(title_disp).add_attribute(Attribute::Bold),
            Cell::new(entry.username),
            Cell::new(entry.url),
            Cell::new(tags_disp).fg(Color::Yellow),
            Cell::new(totp_disp),
        ]));
    }

    println!("{table}");
    Ok(())
}

async fn handle_get(
    vault_path: &Path,
    args: &GetArgs,
    password: Option<String>,
    keyfile: Option<&Path>,
    json: bool,
) -> Result<()> {
    let entry: DecryptedEntry = if let Some(IpcResponse::GetEntry(e)) = try_ipc_request(&IpcRequest::GetEntry { query: args.query.clone() }).await {
        e
    } else {
        let manager = open_vault(vault_path, password, keyfile)?;
        let entry_id = resolve_entry_id(&manager, &args.query)?;
        manager.get_entry(entry_id)?
    };

    if args.copy {
        let secret = Zeroizing::new(entry.password.clone());
        let _ = copy_to_clipboard_defended(&secret, true, None);
        println!("{} Password for '{}' copied to defended clipboard!", "✓".green().bold(), entry.title);
        return Ok(());
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&entry).unwrap());
        return Ok(());
    }

    println!("{}", "═════════════════════════════════════════════".blue());
    println!("  {}", entry.title.bold().yellow());
    println!("{}", "═════════════════════════════════════════════".blue());
    println!("  {:18} {}", "UUID:".bold(), entry.id);
    println!("  {:18} {}", "Username:".bold(), if entry.username.is_empty() { "-" } else { &entry.username });
    
    if args.show_password {
        println!("  {:18} {}", "Password:".bold(), entry.password.green().bold());
    } else {
        println!("  {:18} {}", "Password:".bold(), "••••••••••••••••  (Use --show-password or --copy)".dimmed());
    }

    if !entry.url.is_empty() {
        println!("  {:18} {}", "URL:".bold(), entry.url.cyan());
    }
    if !entry.email.is_empty() {
        println!("  {:18} {}", "Email:".bold(), entry.email);
    }

    if let Some(totp_secret) = entry.totp_secret.as_ref() {
        if args.totp || args.show_password {
            let config = build_totp_config(totp_secret)?;
            if let Ok(code) = generate_totp(&config) {
                let status_disp = if code.seconds_remaining < 5 {
                    code.code.bold().yellow()
                } else {
                    code.code.bold().green()
                };
                println!("  {:18} {} ({}s remaining)", "TOTP Code:".bold(), status_disp, code.seconds_remaining);
            }
        } else {
            println!("  {:18} {}", "TOTP Secret:".bold(), "[Configured]".green());
        }
    }

    if !entry.tags.is_empty() {
        println!("  {:18} {}", "Tags:".bold(), entry.tags.join(", ").yellow());
    }
    if !entry.notes.is_empty() {
        println!("  {:18}\n{}", "Notes:".bold(), entry.notes.dimmed());
    }
    println!("{}", "═════════════════════════════════════════════".blue());

    Ok(())
}

async fn handle_add(
    vault_path: &Path,
    args: &AddArgs,
    password: Option<String>,
    keyfile: Option<&Path>,
) -> Result<()> {
    if args.entry_password.is_some() {
        eprintln!(
            "{} Security Warning: Passing entry passwords via CLI flags exposes them in system process lists.",
            "⚠️".yellow().bold()
        );
    }

    let pass_val = if args.generate {
        let opts = GeneratorOptions {
            length: 24,
            uppercase: true,
            lowercase: true,
            digits: true,
            symbols: true,
            ..Default::default()
        };
        generate_password(&opts)
    } else if let Some(ref p) = args.entry_password {
        p.clone()
    } else {
        prompt_password("Enter Entry Password (leave empty to generate): ")
            .unwrap_or_default()
    };

    let final_password = if pass_val.is_empty() {
        let opts = GeneratorOptions {
            length: 24,
            uppercase: true,
            lowercase: true,
            digits: true,
            symbols: true,
            ..Default::default()
        };
        generate_password(&opts)
    } else {
        pass_val
    };

    let tags = args.tags.as_ref().map(|t| t.split(',').map(|s| s.trim().to_string()).collect()).unwrap_or_default();

    let new_entry = NewEntry {
        title: args.title.clone(),
        username: args.username.clone().unwrap_or_default(),
        password: final_password,
        url: args.url.clone().unwrap_or_default(),
        email: args.email.clone().unwrap_or_default(),
        notes: args.notes.clone().unwrap_or_default(),
        tags,
        totp_secret: args.totp_secret.clone(),
        custom_fields: Vec::new(),
        entry_type: None,
        generate_passkey: None,
        attachments: None,
    };

    // Try IPC first for fast <5ms sync
    if let Some(IpcResponse::AddEntrySuccess(id)) = try_ipc_request(&IpcRequest::AddEntry { new_entry: new_entry.clone() }).await {
        println!("{} Entry '{}' added via IPC session daemon (UUID: {})!", "✓".green().bold(), args.title, id);
        return Ok(());
    }

    let mut manager = open_vault(vault_path, password, keyfile)?;
    let id = manager.add_entry(new_entry)?;
    manager.save()?;

    println!("{} Entry '{}' added successfully (UUID: {})!", "✓".green().bold(), args.title, id);
    Ok(())
}

async fn handle_edit(
    vault_path: &Path,
    args: &EditArgs,
    password: Option<String>,
    keyfile: Option<&Path>,
) -> Result<()> {
    if args.entry_password.is_some() {
        eprintln!(
            "{} Security Warning: Passing entry passwords via CLI flags exposes them in system process lists.",
            "⚠️".yellow().bold()
        );
    }

    let tags = args.tags.as_ref().map(|t| t.split(',').map(|s| s.trim().to_string()).collect());

    let update = UpdateEntry {
        title: args.title.clone(),
        username: args.username.clone(),
        password: args.entry_password.clone(),
        url: args.url.clone(),
        email: args.email.clone(),
        notes: args.notes.clone(),
        tags,
        favorite: None,
        pinned: None,
        totp_secret: args.totp_secret.clone(),
        custom_fields: None,
        breach_status: None,
        passkey_action: None,
        new_attachments: None,
        delete_attachment_ids: None,
    };

    // Try IPC first
    if let Some(IpcResponse::GetEntry(existing)) = try_ipc_request(&IpcRequest::GetEntry { query: args.query.clone() }).await
        && let Some(IpcResponse::UpdateEntrySuccess) = try_ipc_request(&IpcRequest::UpdateEntry { id: existing.id, update: update.clone() }).await {
            println!("{} Entry updated via IPC session daemon!", "✓".green().bold());
            return Ok(());
        }

    let mut manager = open_vault(vault_path, password, keyfile)?;
    let entry_id = resolve_entry_id(&manager, &args.query)?;
    manager.update_entry(entry_id, update)?;
    manager.save()?;

    println!("{} Entry updated successfully!", "✓".green().bold());
    Ok(())
}

async fn handle_delete(
    vault_path: &Path,
    args: &DeleteArgs,
    password: Option<String>,
    keyfile: Option<&Path>,
) -> Result<()> {
    // Try IPC first
    if let Some(IpcResponse::GetEntry(existing)) = try_ipc_request(&IpcRequest::GetEntry { query: args.query.clone() }).await
        && let Some(IpcResponse::DeleteEntrySuccess) = try_ipc_request(&IpcRequest::DeleteEntry { id: existing.id, permanent: args.permanent }).await {
            if args.permanent {
                println!("{} Entry permanently deleted via IPC daemon!", "✓".red().bold());
            } else {
                println!("{} Entry moved to trash via IPC daemon!", "✓".yellow().bold());
            }
            return Ok(());
        }

    let mut manager = open_vault(vault_path, password, keyfile)?;
    let entry_id = resolve_entry_id(&manager, &args.query)?;

    if args.permanent {
        manager.permanent_delete(entry_id)?;
        println!("{} Entry permanently deleted!", "✓".red().bold());
    } else {
        manager.delete_entry(entry_id)?;
        println!("{} Entry moved to trash!", "✓".yellow().bold());
    }

    manager.save()?;
    Ok(())
}

async fn handle_totp(
    vault_path: &Path,
    args: &TotpArgs,
    password: Option<String>,
    keyfile: Option<&Path>,
) -> Result<()> {
    if (args.query.len() >= 16 && !args.query.contains(' ')) || args.query.starts_with("otpauth://") {
        let config = build_totp_config(&args.query)?;
        let code = generate_totp(&config)?;
        println!("{} TOTP Code: {} (expires in {}s)", "🔑".bold(), code.code.bold().green(), code.seconds_remaining);
        return Ok(());
    }

    let entry = if let Some(IpcResponse::GetEntry(e)) = try_ipc_request(&IpcRequest::GetEntry { query: args.query.clone() }).await {
        e
    } else {
        let manager = open_vault(vault_path, password, keyfile)?;
        let entry_id = resolve_entry_id(&manager, &args.query)?;
        manager.get_entry(entry_id)?
    };

    let totp_secret = entry.totp_secret.ok_or_else(|| {
        VaultError::InvalidFormat(format!("Entry '{}' does not have TOTP configured", entry.title))
    })?;

    let config = build_totp_config(&totp_secret)?;
    let code = generate_totp(&config)?;
    
    if args.copy {
        let secret = Zeroizing::new(code.code.clone());
        let _ = copy_to_clipboard_defended(&secret, true, None);
        println!("{} TOTP code [{}] copied to defended clipboard!", "✓".green().bold(), code.code);
    } else {
        if code.seconds_remaining < 5 {
            println!("{} TOTP for '{}': {} (⚠️ Expires in {}s!)", "🔑".bold(), entry.title, code.code.bold().yellow(), code.seconds_remaining);
        } else {
            println!("{} TOTP for '{}': {} ({}s remaining)", "🔑".bold(), entry.title, code.code.bold().green(), code.seconds_remaining);
        }
    }

    Ok(())
}

fn handle_generate(args: &GenerateArgs) -> Result<()> {
    if let Some(kf_path) = &args.keyfile {
        VaultManager::generate_key_file(kf_path)?;
        println!("{} Generated 32-byte cryptographic keyfile at {}", "✓".green().bold(), kf_path.display().to_string().yellow());
        return Ok(());
    }

    let password = if args.diceware {
        let opts = GeneratorOptions {
            mode: GeneratorMode::Diceware,
            word_count: args.words,
            ..Default::default()
        };
        generate_password(&opts)
    } else {
        let opts = GeneratorOptions {
            mode: GeneratorMode::Random,
            length: args.length,
            uppercase: !args.no_uppercase,
            lowercase: !args.no_lowercase,
            digits: !args.no_numbers,
            symbols: !args.no_symbols,
            ..Default::default()
        };
        generate_password(&opts)
    };

    let score = analyze_password(&password);

    if args.copy {
        let secret = Zeroizing::new(password.clone());
        let _ = copy_to_clipboard_defended(&secret, true, None);
        println!("{} Generated password copied to defended clipboard!", "✓".green().bold());
    } else {
        println!("{} Generated Password: {}", "🔑".bold(), password.bold().green());
        println!("   {:18} {:.1} bits ({:?})", "Strength Entropy:".bold(), score.entropy_bits, score.level);
    }

    Ok(())
}

async fn handle_search(
    vault_path: &Path,
    args: &SearchArgs,
    password: Option<String>,
    keyfile: Option<&Path>,
    json: bool,
) -> Result<()> {
    let results: Vec<EntryPreview> = if let Some(IpcResponse::SearchEntries(res)) = try_ipc_request(&IpcRequest::SearchEntries { query: args.query.clone() }).await {
        res
    } else {
        let manager = open_vault(vault_path, password, keyfile)?;
        manager.search_entries(&args.query)?
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&results).unwrap());
        return Ok(());
    }

    if results.is_empty() {
        println!("{}", "No entries matched search query.".yellow());
        return Ok(());
    }

    let mut table = Table::new();
    table.set_header(vec!["UUID", "Title", "Username", "URL"]);

    for entry in results {
        table.add_row(Row::from(vec![
            entry.id.to_string()[..8].to_string(),
            entry.title,
            entry.username,
            entry.url,
        ]));
    }

    println!("{table}");
    Ok(())
}

async fn handle_audit(
    vault_path: &Path,
    password: Option<String>,
    keyfile: Option<&Path>,
    json: bool,
) -> Result<()> {
    let audit: SecurityAudit = if let Some(IpcResponse::AuditSuccess(a)) = try_ipc_request(&IpcRequest::SecurityAudit).await {
        a
    } else {
        let manager = open_vault(vault_path, password, keyfile)?;
        manager.security_audit()?
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&audit).unwrap());
        return Ok(());
    }

    println!("{}", "═════════════════════════════════════════════".blue());
    println!("  {}", "SECURITY AUDIT REPORT".bold().yellow());
    println!("{}", "═════════════════════════════════════════════".blue());
    println!("  {:20} {}", "Overall Score:".bold(), format!("{}/100", audit.health_score).bold().green());
    println!("  {:20} {}", "Total Entries:".bold(), audit.total_entries);
    println!("  {:20} {}", "Weak Passwords:".bold(), if audit.weak_count > 0 { audit.weak_count.to_string().red().bold() } else { "0".green() });
    println!("  {:20} {}", "Reused Passwords:".bold(), if audit.reused_count > 0 { audit.reused_count.to_string().red().bold() } else { "0".green() });
    println!("  {:20} {}", "Old Passwords:".bold(), if audit.old_count > 0 { audit.old_count.to_string().yellow() } else { "0".green() });
    println!("  {:20} {}", "Breached Passwords:".bold(), if audit.breached_count > 0 { audit.breached_count.to_string().red().bold() } else { "0".green() });
    println!("{}", "═════════════════════════════════════════════".blue());

    Ok(())
}

async fn handle_check_breach(args: &CheckBreachArgs) -> Result<()> {
    let pass = match args.password {
        Some(ref p) => p.clone(),
        None => prompt_password("Enter Password to Check Against HIBP API: ")
            .map_err(|e| VaultError::InvalidFormat(format!("Input error: {}", e)))?,
    };

    let result = check_password_breach(&pass).await?;
    if result.is_breached {
        println!("{} WARNING: Password found in {} public data breaches!", "⚠️".red().bold(), result.breach_count);
    } else {
        println!("{} SAFE: Password not found in known breach databases.", "✓".green().bold());
    }

    Ok(())
}

fn handle_import(
    vault_path: &Path,
    args: &ImportArgs,
    password: Option<String>,
    keyfile: Option<&Path>,
) -> Result<()> {
    let mut manager = open_vault(vault_path, password, keyfile)?;
    
    let strategy = match args.on_duplicate.to_lowercase().as_str() {
        "skip" => DuplicateStrategy::Skip,
        "overwrite" => DuplicateStrategy::Overwrite,
        "keep-both" | "duplicate" => DuplicateStrategy::KeepBoth,
        other => return Err(VaultError::InvalidFormat(format!("Invalid --on-duplicate strategy '{}'. Allowed: skip, overwrite, keep-both", other))),
    };

    let preview = Importer::parse_file(&args.file, ImportFormat::AutoDetect)?;
    println!("{} Discovered {} valid entries to import from {}", "→".blue(), preview.entries.len(), args.file.display());

    let count = manager.bulk_import_entries(preview.entries, strategy)?;
    println!("{} Successfully imported {} entries (strategy: {})!", "✓".green().bold(), count, args.on_duplicate);
    Ok(())
}

fn handle_export(
    vault_path: &Path,
    args: &ExportArgs,
    password: Option<String>,
    keyfile: Option<&Path>,
) -> Result<()> {
    let fmt_clean = args.format.to_lowercase();
    if (fmt_clean == "csv" || fmt_clean == "json") && !args.force {
        return Err(VaultError::InvalidFormat(
            "Exporting unencrypted credentials in plaintext requires the --force flag to confirm!".into()
        ));
    }

    let manager = open_vault(vault_path, password, keyfile)?;

    match fmt_clean.as_str() {
        "vdb" => {
            std::fs::copy(vault_path, &args.dest)
                .map_err(|e| VaultError::VaultNotFound(format!("Export file write error: {}", e)))?;
        }
        "csv" => {
            manager.export_csv(&args.dest)?;
        }
        "json" => {
            manager.export_json(&args.dest)?;
        }
        fmt => return Err(VaultError::InvalidFormat(format!("Unsupported export format: {}", fmt))),
    }

    println!("{} Exported vault contents to {} ({})", "✓".green().bold(), args.dest.display(), args.format);
    Ok(())
}

fn handle_recovery(path:&Path,args:&RecoveryArgs,password:Option<String>,keyfile:Option<&Path>) -> Result<()> {
    match &args.action {
        RecoveryAction::Generate{output_dir} => {
            std::fs::create_dir_all(output_dir)?;
            let password=acquire_password(password)?;
            let mut manager=VaultManager::open_with_keyfile(path,&password,keyfile)?;
            let kf=keyfile.map(yntra_vault_core::vault::manager::read_key_file_safely).transpose()?;
            let kit=manager.generate_emergency_kit_with_keyfile(&password,kf.as_ref().map(|b|b.as_slice()))?;
            for share in &kit.shares {
                let output=output_dir.join(format!("recovery-{}-share-{}.txt",kit.verification_hash,share.share_index));
                let mut options=std::fs::OpenOptions::new();options.write(true).create_new(true);
                #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
                let mut file=options.open(output)?;
                writeln!(file,"Yntra recovery v2 — {} — share {}\n\n{}\n\n{}",kit.vault_name,share.share_index,share.share_data,kit.document_markdown)?;
                file.sync_all()?;
            }
            println!("Saved three separate recovery files. Move them to separate safe locations. Old kits still work with old backups.");
        }
        RecoveryAction::Revoke => {
            let password=acquire_password(password)?;let mut manager=VaultManager::open_with_keyfile(path,&password,keyfile)?;
            let kf=keyfile.map(yntra_vault_core::vault::manager::read_key_file_safely).transpose()?;
            manager.revoke_emergency_kit(&password,kf.as_ref().map(|b|b.as_slice()))?;
            println!("Recovery revoked for the current file.");
        }
        RecoveryAction::Restore{share_a_file,share_b_file} => {
            fn read_share(path:&Path)->Result<Zeroizing<String>>{
                if std::fs::metadata(path)?.len()>8192{return Err(VaultError::InvalidFormat("Recovery file too large".into()));}
                let content=Zeroizing::new(std::fs::read_to_string(path)?);
                content.lines().map(str::trim).find(|line|line.starts_with("YNTRA2:")||line.starts_with("YNTRA-SHARE")||line.starts_with("SL-SHARE")).map(|line|Zeroizing::new(line.to_owned())).ok_or_else(||VaultError::InvalidFormat("No recovery share found".into()))
            }
            let a=read_share(share_a_file)?;let b=read_share(share_b_file)?;
            let password=Zeroizing::new(prompt_password("New master password (12+ characters): ")?);
            let confirmation=Zeroizing::new(prompt_password("Confirm new password: ")?);
            if *password!=*confirmation{return Err(VaultError::InvalidPassword);}
            VaultManager::recover_with_shares(path,&a,&b,&password)?;
            println!("Access restored. USB binding removed and recovery kit consumed for this file. Create a new kit before binding USB again.");
        }
    }
    Ok(())
}

fn handle_shamir(args: &ShamirArgs) -> Result<()> {
    match &args.action {
        ShamirAction::Split { password } => {
            let pass = acquire_password(password.clone())?;
            let pass_hash = Sha256::digest(pass.as_bytes());
            let shares = split_secret(&pass_hash)?;
            println!("{}", "═════════════════════════════════════════════".blue());
              println!("  {}", "LEGACY HASH SHARES — NOT A VAULT RECOVERY KIT".bold().yellow());
            println!("{}", "═════════════════════════════════════════════".blue());
            for (i, share) in shares.iter().enumerate() {
                println!("  Share {}: {}", i + 1, share.bold().green());
            }
            println!("{}", "═════════════════════════════════════════════".blue());
            println!("  Store these shares in separate secure locations.");
            println!("  Any 2 shares are required to reconstruct recovery key.");
        }
        ShamirAction::Recover { share_a, share_b } => {
            let raw_bytes = reconstruct_secret(share_a, share_b)?;
            let hash_hex = data_encoding::HEXLOWER.encode(&raw_bytes);
            println!("{} Password SHA-256 Hash Reconstructed: {}", "✓".green().bold(), hash_hex.bold().yellow());
        }
    }
    Ok(())
}

async fn handle_sync(
    vault_path: &Path,
    args: &SyncArgs,
    password: Option<String>,
    keyfile: Option<&Path>,
) -> Result<()> {
    match &args.action {
        SyncAction::WebdavUpload { url, username, password: webdav_pass } => {
            let etag = webdav_upload(url, username, webdav_pass.as_deref(), vault_path, None).await?;
            println!("{} WebDAV Upload Complete! Remote ETag: {}", "✓".green().bold(), etag.unwrap_or_default());
        }
        SyncAction::WebdavDownload { url, username, password: webdav_pass } => {
            webdav_download(url, username, webdav_pass.as_deref(), vault_path).await?;
            println!("{} WebDAV Download Complete! Saved to {}", "✓".green().bold(), vault_path.display());
        }
        SyncAction::P2pListen { listen } => {
            let mut manager = open_vault(vault_path, password, keyfile)?;
            let snapshot = yntra_vault_core::services::sync::SyncSnapshot::from_manager(&manager)?;
            let subkeys = manager.get_subkeys()?;
            println!("{} Starting P2P Sync Server on {}...", "→".blue().bold(), listen);
            let (stats, data) = run_p2p_sync_listener(listen, subkeys, &snapshot.path())?;
            yntra_vault_core::services::sync::merge_vault_data(&mut manager.data,data);manager.save()?;
            println!("{} P2P Sync Completed! Added: {}, Updated: {}, Retained: {}", "✓".green().bold(), stats.entries_added, stats.entries_updated, stats.entries_kept_local);
        }
        SyncAction::P2pConnect { server } => {
            let mut manager = open_vault(vault_path, password, keyfile)?;
            let snapshot = yntra_vault_core::services::sync::SyncSnapshot::from_manager(&manager)?;
            let subkeys = manager.get_subkeys()?;
            let (stats, data) = run_p2p_sync_client(server, subkeys, &snapshot.path())?;
            yntra_vault_core::services::sync::merge_vault_data(&mut manager.data,data);manager.save()?;
            println!("{} P2P Sync Completed! Added: {}, Updated: {}, Retained: {}", "✓".green().bold(), stats.entries_added, stats.entries_updated, stats.entries_kept_local);
        }
        SyncAction::PairHost { listen, code } => {
            let pass = acquire_password(password)?;
            let pin = match code {
                Some(c) => c.clone(),
                None => {
                    let generated_pin = generate_pairing_code();
                    println!("{} Generated Pairing Code: {}", "🔑".yellow().bold(), generated_pin.bold().green());
                    generated_pin
                }
            };
            println!("{} Starting Zero-Knowledge Device Pairing Host on {}...", "→".blue().bold(), listen);
            println!("{} Enter PIN '{}' on your other device to connect and merge vaults.", "ℹ".cyan().bold(), pin);
            let (stats, _) = run_p2p_pairing_host(listen, &pass, &pin, vault_path, std::time::Duration::from_secs(60))?;
            println!("{} Device Pairing Completed! Sent: {}, Received: {}, Merged: {}, Total: {}", "✓".green().bold(), stats.entries_sent, stats.entries_received, stats.entries_merged, stats.total_entries);
        }
        SyncAction::PairConnect { code, server } => {
            let pass = acquire_password(password)?;
            let server_addr = match server {
                Some(s) => s.clone(),
                None => {
                    println!("{} Scanning local Wi-Fi for pairing host matching PIN '{}'...", "📡".cyan().bold(), code);
                    let discovery_id = compute_pairing_beacon_id(&pass, code)?;
                    match listen_pairing_beacon(&discovery_id, std::time::Duration::from_secs(5))? {
                        Some(addr) => {
                            println!("{} Found pairing host at {}!", "✓".green().bold(), addr);
                            addr.to_string()
                        }
                        None => {
                            return Err(VaultError::SyncError("No matching host device found on local Wi-Fi. Check that both devices are on the same network.".into()));
                        }
                    }
                }
            };
            println!("{} Connecting to pairing host at {}...", "→".blue().bold(), server_addr);
            let (stats, _) = run_p2p_pairing_client(&server_addr, &pass, code, vault_path)?;
            println!("{} Device Pairing Completed! Sent: {}, Received: {}, Merged: {}, Total: {}", "✓".green().bold(), stats.entries_sent, stats.entries_received, stats.entries_merged, stats.total_entries);
        }
    }
    Ok(())
}

async fn handle_autotype(
    vault_path: &Path,
    args: &AutotypeArgs,
    password: Option<String>,
    keyfile: Option<&Path>,
) -> Result<()> {
    let entry = if let Some(IpcResponse::GetEntry(e)) = try_ipc_request(&IpcRequest::GetEntry { query: args.query.clone() }).await {
        e
    } else {
        let manager = open_vault(vault_path, password, keyfile)?;
        let entry_id = resolve_entry_id(&manager, &args.query)?;
        manager.get_entry(entry_id)?
    };

    println!("{} Target: '{}'. Focus the target input field!", "⌛".yellow().bold(), entry.title);
    print!("Autotyping in: ");
    for i in (1..=3).rev() {
        print!("{}... ", i);
        let _ = std::io::stdout().flush();
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
    println!();

    run_smart_autotype(
        entry.username,
        entry.password,
    )?;

    println!("{} Smart autotype executed!", "✓".green().bold());
    Ok(())
}

fn handle_change_password(
    vault_path: &Path,
    args: &ChangePasswordArgs,
    password: Option<String>,
    keyfile: Option<&Path>,
) -> Result<()> {
    let mut manager = open_vault(vault_path, password.clone(), keyfile)?;

    let current_pass = acquire_password(password)?;

    let new_pass = match args.new_password {
        Some(ref p) => p.clone(),
        None => {
            let input = prompt_password("Enter New Master Password: ")
                .map_err(|e| VaultError::InvalidFormat(format!("Input error: {}", e)))?;
            let confirm = prompt_password("Confirm New Master Password: ")
                .map_err(|e| VaultError::InvalidFormat(format!("Input error: {}", e)))?;
            if input != confirm {
                return Err(VaultError::InvalidFormat("New passwords do not match!".into()));
            }
            input
        }
    };

    if keyfile.is_some() || args.new_keyfile.is_some() {
        manager.change_master_password_with_keyfiles(&current_pass, keyfile, &new_pass, args.new_keyfile.as_deref())?;
    } else {
        manager.change_master_password(&current_pass, &new_pass)?;
    }

    println!("{} Master password changed successfully!", "✓".green().bold());
    Ok(())
}

fn handle_biometric(
    vault_path: &Path,
    args: &BiometricArgs,
    password: Option<String>,
    keyfile: Option<&Path>,
) -> Result<()> {
    match &args.action {
        BiometricAction::Status => {
            let info = check_biometric_availability();
            println!("Biometric hardware available: {} ({})", if info.available { "YES".green().bold() } else { "NO".red() }, info.biometric_type);
        }
        BiometricAction::Enable => {
            let mut manager = open_vault(vault_path, password, keyfile)?;
            manager.enable_biometric()?;
            println!("{} Biometric unlock enabled for vault header!", "✓".green().bold());
        }
        BiometricAction::Disable => {
            let mut manager = open_vault(vault_path, password, keyfile)?;
            manager.disable_biometric()?;
            println!("{} Biometric unlock disabled!", "✓".yellow().bold());
        }
    }
    Ok(())
}

async fn handle_update(args: UpdateArgs, json: bool) -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    let target_platform = if cfg!(windows) {
        "windows-cli"
    } else {
        "linux-cli"
    };

    if !json {
        println!("{}", "Checking for Yntra Vault updates...".cyan());
    }

    let check_result = yntra_vault_core::services::updater::check_for_updates(
        current_version,
        target_platform,
        args.endpoint.as_deref(),
    )
    .await?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&check_result).map_err(VaultError::JsonError)?
        );
        return Ok(());
    }

    if !check_result.has_update {
        println!(
            "{} Yntra Vault CLI is up to date (v{}).",
            "✓".green().bold(),
            current_version
        );
        return Ok(());
    }

    println!(
        "\n{} Newer version available: {} (current: v{})",
        "★".yellow().bold(),
        check_result.latest_version.bold().green(),
        current_version
    );

    if let Some(ref notes) = check_result.release_notes {
        println!("\n{}", "Release Notes:".bold().underline());
        let count = notes.lines().count();
        for line in notes.lines().take(15) {
            println!("  {}", line);
        }
        if count > 15 {
            println!("  ... (see GitHub release for full notes)");
        }
    }

    if args.check {
        return Ok(());
    }

    let download_url = match check_result.download_url {
        Some(url) => url,
        None => {
            eprintln!(
                "{}",
                "No prebuilt CLI binary available for this platform in latest release.".red()
            );
            return Ok(());
        }
    };

    if !args.yes {
        print!(
            "\nDo you want to update to v{} now? [y/N]: ",
            check_result.latest_version
        );
        std::io::stdout().flush().map_err(VaultError::IoError)?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).map_err(VaultError::IoError)?;
        if !input.trim().eq_ignore_ascii_case("y") && !input.trim().eq_ignore_ascii_case("yes") {
            println!("Update cancelled.");
            return Ok(());
        }
    }

    println!("Downloading {}...", download_url.cyan());
    let new_bytes = yntra_vault_core::services::updater::download_file(&download_url).await?;

    let expected_sha = match check_result.sha256 {
        Some(ref s) if !s.trim().is_empty() => s.trim(),
        _ => {
            return Err(VaultError::UpdateError(
                "Update aborted: Mandatory SHA-256 checksum missing from update manifest.".to_string(),
            ));
        }
    };

    print!("Verifying cryptographic integrity (SHA-256)... ");
    if !yntra_vault_core::services::updater::verify_sha256(&new_bytes, expected_sha) {
        eprintln!("{}", "FAILED".red().bold());
        return Err(VaultError::UpdateError(
            "Downloaded binary SHA-256 hash does not match published manifest!".to_string(),
        ));
    }
    println!("{}", "OK".green().bold());

    print!("Applying update to current binary... ");
    replace_current_exe(&new_bytes).map_err(VaultError::IoError)?;
    println!("{}", "DONE".green().bold());

    save_update_cache(&check_result.latest_version, false);

    println!(
        "\n{} Successfully upgraded yntra CLI to v{}!",
        "✓".green().bold(),
        check_result.latest_version.green().bold()
    );

    Ok(())
}

fn replace_current_exe(new_bytes: &[u8]) -> std::io::Result<()> {
    yntra_vault_core::services::updater::replace_executable(&std::env::current_exe()?, new_bytes)
}

fn cleanup_old_binary_if_present() {
    if let Ok(current_exe) = std::env::current_exe() {
        let old_exe = current_exe.with_extension("exe.old");
        if old_exe.exists() {
            let _ = std::fs::remove_file(&old_exe);
        }
    }
}

fn get_update_cache_path() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|p| p.join("YntraVault").join("cli_update_cache.json"))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
            .map(|p| p.join("yntra").join("update_cache.json"))
    }
}

#[derive(Serialize, Deserialize)]
struct CliUpdateCache {
    last_check_timestamp: u64,
    latest_version: String,
    has_update: bool,
}

fn save_update_cache(latest_version: &str, has_update: bool) {
    if let Some(path) = get_update_cache_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let cache = CliUpdateCache {
            last_check_timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            latest_version: latest_version.to_string(),
            has_update,
        };
        if let Ok(serialized) = serde_json::to_string(&cache) {
            let _ = std::fs::write(path, serialized);
        }
    }
}

async fn maybe_print_update_tip() {
    let current_version = env!("CARGO_PKG_VERSION");
    let cache_path = match get_update_cache_path() {
        Some(p) => p,
        None => return,
    };

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    if cache_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&cache_path)
            && let Ok(cache) = serde_json::from_str::<CliUpdateCache>(&content) {
            if cache.has_update && yntra_vault_core::services::updater::is_newer_version(current_version, &cache.latest_version) {
                eprintln!(
                    "{} Yntra Vault v{} is available. Run '{}' to upgrade.",
                    "💡 Tip:".yellow().bold(),
                    cache.latest_version.green(),
                    "yntra update".cyan()
                );
            }
            if now_secs.saturating_sub(cache.last_check_timestamp) < 86400 {
                return;
            }
        }
    }

    let target_platform = if cfg!(windows) { "windows-cli" } else { "linux-cli" };
    if let Ok(Ok(result)) = tokio::time::timeout(
        Duration::from_secs(3),
        yntra_vault_core::services::updater::check_for_updates(current_version, target_platform, None),
    ).await {
        save_update_cache(&result.latest_version, result.has_update);
        if result.has_update {
            eprintln!(
                "{} Yntra Vault v{} is available. Run '{}' to upgrade.",
                "💡 Tip:".yellow().bold(),
                result.latest_version.green(),
                "yntra update".cyan()
            );
        }
    }
}
