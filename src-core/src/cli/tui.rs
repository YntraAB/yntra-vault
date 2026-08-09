//! Yntra Vault — Interactive Terminal UI (`yntra tui`)
//!
//! Ratatui + Crossterm interactive terminal vault explorer with live search,
//! Vim navigation, TOTP countdown, entry creation dialogs, delete confirmation modals,
//! and defended clipboard copy shortcuts.

use std::io::stdout;
use std::path::Path;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap, Clear},
    Terminal,
};
use zeroize::Zeroizing;

use crate::{
    Result, VaultError,
    vault::{EntryPreview, manager::{NewEntry, DecryptedEntry}},
    totp::{generate_totp, parse_otpauth_uri, TotpConfig},
    crypto::clipboard::copy_to_clipboard_defended,
};
use crate::cli::ipc::{try_ipc_request, IpcRequest, IpcResponse};

pub async fn run_tui(
    vault_path: &Path,
    password: Option<String>,
    keyfile: Option<&Path>,
) -> Result<()> {
    let entries = if let Some(IpcResponse::ListEntries(list)) = try_ipc_request(&IpcRequest::ListEntries).await {
        list
    } else {
        let pass = password.ok_or_else(|| VaultError::VaultLocked)?;
        let manager = crate::vault::VaultManager::open_with_keyfile(vault_path, &pass, keyfile)?;
        manager.list_entries()?
    };

    enable_raw_mode().map_err(|e| VaultError::InvalidFormat(format!("Raw mode error: {}", e)))?;
    let mut stdout_handle = stdout();
    execute!(stdout_handle, EnterAlternateScreen).map_err(|e| VaultError::InvalidFormat(format!("Screen setup error: {}", e)))?;
    let backend = CrosstermBackend::new(stdout_handle);
    let mut terminal = Terminal::new(backend).map_err(|e| VaultError::InvalidFormat(format!("Terminal init error: {}", e)))?;

    let mut state = AppState {
        entries,
        filtered_indices: Vec::new(),
        list_state: ListState::default(),
        search_query: String::new(),
        mode: Mode::Normal,
        show_password: false,
        active_decrypted: None,
        status_msg: String::from("Ready — [/] Search | [a] Add | [d] Delete | [c] Copy Pass | [t] Copy TOTP | [q] Quit"),
        create_title: String::new(),
        create_user: String::new(),
        create_pass: String::new(),
        create_url: String::new(),
        create_step: 0,
    };

    state.update_filter();
    if !state.filtered_indices.is_empty() {
        state.list_state.select(Some(0));
        state.fetch_active_detail().await;
    }

    let res = main_loop(&mut terminal, &mut state).await;

    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    res
}

#[derive(PartialEq)]
enum Mode {
    Normal,
    Searching,
    Creating,
    ConfirmDelete,
}

struct AppState {
    entries: Vec<EntryPreview>,
    filtered_indices: Vec<usize>,
    list_state: ListState,
    search_query: String,
    mode: Mode,
    show_password: bool,
    active_decrypted: Option<DecryptedEntry>,
    status_msg: String,
    create_title: String,
    create_user: String,
    create_pass: String,
    create_url: String,
    create_step: usize,
}

impl AppState {
    fn update_filter(&mut self) {
        let q = self.search_query.to_lowercase();
        self.filtered_indices = self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                q.is_empty()
                    || e.title.to_lowercase().contains(&q)
                    || e.username.to_lowercase().contains(&q)
                    || e.url.to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect();
    }

    async fn refresh_entries(&mut self) {
        if let Some(IpcResponse::ListEntries(list)) = try_ipc_request(&IpcRequest::ListEntries).await {
            self.entries = list;
            self.update_filter();
        }
    }

    async fn fetch_active_detail(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if selected < self.filtered_indices.len() {
                let real_idx = self.filtered_indices[selected];
                let entry_id = self.entries[real_idx].id;

                if let Some(IpcResponse::GetEntry(e)) = try_ipc_request(&IpcRequest::GetEntry { query: entry_id.to_string() }).await {
                    self.active_decrypted = Some(e);
                }
            }
        }
    }
}

async fn main_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    state: &mut AppState,
) -> Result<()> {
    loop {
        terminal.draw(|f| draw_ui(f, state)).map_err(|e| VaultError::InvalidFormat(format!("Draw error: {}", e)))?;

        if event::poll(Duration::from_millis(250)).map_err(|e| VaultError::InvalidFormat(format!("Poll error: {}", e)))? {
            if let Event::Key(key) = event::read().map_err(|e| VaultError::InvalidFormat(format!("Read error: {}", e)))? {
                match state.mode {
                    Mode::Searching => match key.code {
                        KeyCode::Esc | KeyCode::Enter => state.mode = Mode::Normal,
                        KeyCode::Backspace => {
                            state.search_query.pop();
                            state.update_filter();
                            state.list_state.select(Some(0));
                            state.fetch_active_detail().await;
                        }
                        KeyCode::Char(c) => {
                            state.search_query.push(c);
                            state.update_filter();
                            state.list_state.select(Some(0));
                            state.fetch_active_detail().await;
                        }
                        _ => {}
                    },
                    Mode::ConfirmDelete => match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                            if let Some(ref e) = state.active_decrypted {
                                let id = e.id;
                                let title = e.title.clone();
                                if let Some(IpcResponse::DeleteEntrySuccess) = try_ipc_request(&IpcRequest::DeleteEntry { id, permanent: false }).await {
                                    state.status_msg = format!("✓ Entry '{}' moved to trash!", title);
                                    state.refresh_entries().await;
                                    state.fetch_active_detail().await;
                                }
                            }
                            state.mode = Mode::Normal;
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                            state.mode = Mode::Normal;
                            state.status_msg = String::from("Deletion cancelled.");
                        }
                        _ => {}
                    },
                    Mode::Creating => match key.code {
                        KeyCode::Esc => {
                            state.mode = Mode::Normal;
                            state.create_step = 0;
                        }
                        KeyCode::Tab | KeyCode::Down => state.create_step = (state.create_step + 1) % 4,
                        KeyCode::Up => state.create_step = if state.create_step == 0 { 3 } else { state.create_step - 1 },
                        KeyCode::Enter => {
                            if !state.create_title.is_empty() {
                                let new_entry = NewEntry {
                                    title: state.create_title.clone(),
                                    username: state.create_user.clone(),
                                    password: state.create_pass.clone(),
                                    url: state.create_url.clone(),
                                    email: String::new(),
                                    notes: String::new(),
                                    tags: Vec::new(),
                                    totp_secret: None,
                                    custom_fields: Vec::new(),
                                    entry_type: None,
                                    generate_passkey: None,
                                    attachments: None,
                                };

                                if let Some(IpcResponse::AddEntrySuccess(_)) = try_ipc_request(&IpcRequest::AddEntry { new_entry }).await {
                                    state.status_msg = format!("✓ Entry '{}' added successfully!", state.create_title);
                                    state.refresh_entries().await;
                                }

                                state.create_title.clear();
                                state.create_user.clear();
                                state.create_pass.clear();
                                state.create_url.clear();
                                state.create_step = 0;
                                state.mode = Mode::Normal;
                            }
                        }
                        KeyCode::Backspace => match state.create_step {
                            0 => { state.create_title.pop(); },
                            1 => { state.create_user.pop(); },
                            2 => { state.create_pass.pop(); },
                            3 => { state.create_url.pop(); },
                            _ => {}
                        },
                        KeyCode::Char(c) => match state.create_step {
                            0 => { state.create_title.push(c); },
                            1 => { state.create_user.push(c); },
                            2 => { state.create_pass.push(c); },
                            3 => { state.create_url.push(c); },
                            _ => {}
                        },
                        _ => {}
                    },
                    Mode::Normal => match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Char('/') => state.mode = Mode::Searching,
                        KeyCode::Char('a') => state.mode = Mode::Creating,
                        KeyCode::Char('s') => state.show_password = !state.show_password,
                        KeyCode::Char('d') => {
                            if state.active_decrypted.is_some() {
                                state.mode = Mode::ConfirmDelete;
                            }
                        }
                        KeyCode::Char('c') => {
                            if let Some(ref e) = state.active_decrypted {
                                let mut sec = Zeroizing::new(e.password.clone());
                                let _ = copy_to_clipboard_defended(&mut sec, true, None);
                                state.status_msg = format!("✓ Password for '{}' copied to defended clipboard!", e.title);
                            }
                        }
                        KeyCode::Char('t') => {
                            if let Some(ref e) = state.active_decrypted {
                                if let Some(totp_sec) = e.totp_secret.as_deref() {
                                    if let Ok(cfg) = parse_totp_config(totp_sec) {
                                        if let Ok(code) = generate_totp(&cfg) {
                                            let mut sec = Zeroizing::new(code.code.clone());
                                            let _ = copy_to_clipboard_defended(&mut sec, true, None);
                                            state.status_msg = format!("✓ TOTP [{}] copied to defended clipboard!", code.code);
                                        }
                                    }
                                }
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if !state.filtered_indices.is_empty() {
                                let curr = state.list_state.selected().unwrap_or(0);
                                let next = (curr + 1) % state.filtered_indices.len();
                                state.list_state.select(Some(next));
                                state.fetch_active_detail().await;
                            }
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if !state.filtered_indices.is_empty() {
                                let curr = state.list_state.selected().unwrap_or(0);
                                let next = if curr == 0 { state.filtered_indices.len() - 1 } else { curr - 1 };
                                state.list_state.select(Some(next));
                                state.fetch_active_detail().await;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn parse_totp_config(sec: &str) -> Result<TotpConfig> {
    if sec.starts_with("otpauth://") {
        parse_otpauth_uri(sec)
    } else {
        Ok(TotpConfig { secret: sec.to_string(), ..Default::default() })
    }
}

fn draw_ui(f: &mut ratatui::Frame, state: &mut AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ].as_ref())
        .split(f.size());

    // 1. Header
    let header_text = vec![
        Line::from(vec![
            Span::styled(" YNTRA VAULT ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled("— High-Security Offline Password Manager TUI", Style::default().fg(Color::Cyan)),
        ])
    ];
    let header_block = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Blue)));
    f.render_widget(header_block, chunks[0]);

    // 2. Main split (Left: Search + Entry List, Right: Details)
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)].as_ref())
        .split(chunks[1]);

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)].as_ref())
        .split(main_chunks[0]);

    // Search bar
    let search_style = if state.mode == Mode::Searching {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let search_para = Paragraph::new(state.search_query.as_str())
        .block(Block::default().borders(Borders::ALL).title(" Search [/] ").border_style(search_style));
    f.render_widget(search_para, left_chunks[0]);

    // Entry List
    let items: Vec<ListItem> = state.filtered_indices
        .iter()
        .map(|&idx| {
            let entry = &state.entries[idx];
            let fav = if entry.favorite { "★ " } else { "  " };
            let totp = if entry.has_totp { " [2FA]" } else { "" };
            let line = format!("{}{:<20}{}", fav, entry.title, totp);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Entries "))
        .highlight_style(Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, left_chunks[1], &mut state.list_state);

    // Right Panel — Entry Details
    let detail_block = Block::default().borders(Borders::ALL).title(" Credentials & Security ");
    if let Some(ref entry) = state.active_decrypted {
        let pass_disp = if state.show_password {
            entry.password.as_str()
        } else {
            "••••••••••••••••  ([s] Toggle Show)"
        };

        let mut lines = vec![
            Line::from(vec![Span::styled("Title:    ", Style::default().add_modifier(Modifier::BOLD)), Span::styled(&entry.title, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))]),
            Line::from(vec![Span::styled("UUID:     ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(entry.id.to_string())]),
            Line::from(vec![Span::styled("Username: ", Style::default().add_modifier(Modifier::BOLD)), Span::styled(&entry.username, Style::default().fg(Color::Cyan))]),
            Line::from(vec![Span::styled("Password: ", Style::default().add_modifier(Modifier::BOLD)), Span::styled(pass_disp, Style::default().fg(Color::Green))]),
            Line::from(vec![Span::styled("URL:      ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(&entry.url)]),
            Line::from(vec![Span::styled("Email:    ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(&entry.email)]),
            Line::from(vec![Span::styled("Tags:     ", Style::default().add_modifier(Modifier::BOLD)), Span::styled(entry.tags.join(", "), Style::default().fg(Color::Yellow))]),
            Line::from(vec![]),
        ];

        if let Some(totp_sec) = entry.totp_secret.as_deref() {
            if let Ok(cfg) = parse_totp_config(totp_sec) {
                if let Ok(code) = generate_totp(&cfg) {
                    let code_str = code.code;
                    let rem_str = format!(" ({}s remaining)", code.seconds_remaining);
                    lines.push(Line::from(vec![
                        Span::styled("TOTP Code: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::styled(code_str, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                        Span::styled(rem_str, Style::default().fg(Color::DarkGray)),
                    ]));
                }
            }
        }

        if !entry.notes.is_empty() {
            lines.push(Line::from(vec![Span::styled("Notes:    ", Style::default().add_modifier(Modifier::BOLD))]));
            lines.push(Line::from(vec![Span::raw(&entry.notes)]));
        }

        let paragraph = Paragraph::new(lines)
            .block(detail_block)
            .wrap(Wrap { trim: true });
        f.render_widget(paragraph, main_chunks[1]);
    } else {
        let empty_para = Paragraph::new("No Entry Selected")
            .block(detail_block)
            .alignment(Alignment::Center);
        f.render_widget(empty_para, main_chunks[1]);
    }

    // Modal delete confirmation dialog
    if state.mode == Mode::ConfirmDelete {
        let area = centered_rect(50, 25, f.size());
        f.render_widget(Clear, area);

        let title_name = state.active_decrypted.as_ref().map(|e| e.title.as_str()).unwrap_or("Entry");
        let prompt_text = vec![
            Line::from(vec![Span::styled("Are you sure you want to move this entry to trash?", Style::default().add_modifier(Modifier::BOLD))]),
            Line::from(vec![Span::styled(title_name, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))]),
            Line::from(vec![]),
            Line::from(vec![Span::styled("Press [Y] to Confirm  |  Press [N/Esc] to Cancel", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))]),
        ];

        let delete_block = Paragraph::new(prompt_text)
            .block(Block::default().title(" Confirm Deleting Entry ").borders(Borders::ALL).border_style(Style::default().fg(Color::Red)))
            .alignment(Alignment::Center);
        f.render_widget(delete_block, area);
    }

    // Modal creation dialog if Creating mode
    if state.mode == Mode::Creating {
        let area = centered_rect(60, 50, f.size());
        f.render_widget(Clear, area);

        let form_block = Block::default()
            .title(" Add New Entry — [Tab/Arrows] Navigate | [Enter] Save | [Esc] Cancel ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));
        f.render_widget(form_block, area);

        let form_chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
            ].as_ref())
            .split(area);

        let s0 = if state.create_step == 0 { Style::default().fg(Color::Yellow) } else { Style::default() };
        let s1 = if state.create_step == 1 { Style::default().fg(Color::Yellow) } else { Style::default() };
        let s2 = if state.create_step == 2 { Style::default().fg(Color::Yellow) } else { Style::default() };
        let s3 = if state.create_step == 3 { Style::default().fg(Color::Yellow) } else { Style::default() };

        f.render_widget(Paragraph::new(state.create_title.as_str()).block(Block::default().borders(Borders::ALL).title(" Title * ").border_style(s0)), form_chunks[0]);
        f.render_widget(Paragraph::new(state.create_user.as_str()).block(Block::default().borders(Borders::ALL).title(" Username ").border_style(s1)), form_chunks[1]);
        f.render_widget(Paragraph::new(state.create_pass.as_str()).block(Block::default().borders(Borders::ALL).title(" Password ").border_style(s2)), form_chunks[2]);
        f.render_widget(Paragraph::new(state.create_url.as_str()).block(Block::default().borders(Borders::ALL).title(" URL ").border_style(s3)), form_chunks[3]);
    }

    // 3. Footer
    let footer_para = Paragraph::new(state.status_msg.as_str())
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
    f.render_widget(footer_para, chunks[2]);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ].as_ref())
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ].as_ref())
        .split(popup_layout[1])[1]
}
