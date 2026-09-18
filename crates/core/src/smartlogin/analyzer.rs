//! Page Analyzer — extracts interactive elements from a web page
//! by combining CDP Accessibility Tree data with DOM metadata via injected JavaScript.

use chromiumoxide::page::Page;
use serde::Deserialize;

use crate::smartlogin::types::*;
use crate::smartlogin::logging::{SmartLoginLogger, SmartLoginEvent, SmartLoginEventDetail};

/// JavaScript injected into the page to extract form/input/button/link metadata.
/// Runs in the page context; returns a JSON blob consumed by the Rust analyzer.
const PAGE_ANALYSIS_JS: &str = r#"
(() => {
    function isVisible(el) {
        if (!el) return false;
        const style = window.getComputedStyle(el);
        if (style.display === 'none' || style.visibility === 'hidden' || style.opacity === '0') return false;
        const rect = el.getBoundingClientRect();
        return rect.width > 0 && rect.height > 0;
    }

    function getLabel(el) {
        if (el.id) {
            const label = document.querySelector('label[for="' + CSS.escape(el.id) + '"]');
            if (label) return label.textContent.trim();
        }
        const parent = el.closest('label');
        if (parent) return parent.textContent.trim();
        return '';
    }

    function getSurroundingText(el) {
        const parent = el.parentElement;
        if (!parent) return '';
        const texts = [];
        for (const node of parent.childNodes) {
            if (node.nodeType === 3) {
                const t = node.textContent.trim();
                if (t) texts.push(t);
            }
        }
        return texts.join(' ').substring(0, 200);
    }

    function getFormIndex(el) {
        const form = el.closest('form');
        if (!form) return null;
        const forms = Array.from(document.querySelectorAll('form'));
        return forms.indexOf(form);
    }

    function inNav(el) {
        return !!el.closest('nav, header, [role="navigation"], [role="banner"]');
    }

    // Collect forms
    const forms = Array.from(document.querySelectorAll('form')).map(f => ({
        form_id: f.id || null,
        action: f.action || '',
        method: (f.method || 'get').toUpperCase(),
        input_count: f.querySelectorAll('input').length,
        has_password_input: f.querySelector('input[type="password"]') !== null,
    }));

    // Collect inputs
    const inputs = Array.from(document.querySelectorAll('input, select')).map(el => ({
        backend_node_id: 0,
        input_type: el.type || el.tagName.toLowerCase(),
        name: el.name || '',
        id: el.id || '',
        placeholder: el.placeholder || '',
        autocomplete: el.getAttribute('autocomplete') || '',
        aria_label: el.getAttribute('aria-label') || '',
        associated_label: getLabel(el),
        is_visible: isVisible(el),
        is_readonly: el.readOnly || el.disabled || false,
        form_index: getFormIndex(el),
        surrounding_text: getSurroundingText(el),
        ax_role: el.getAttribute('role') || '',
        ax_name: '',
    }));

    // Collect buttons (button elements + input[type=submit] + role=button)
    const buttonEls = [
        ...document.querySelectorAll('button'),
        ...document.querySelectorAll('input[type="submit"]'),
        ...document.querySelectorAll('input[type="button"]'),
        ...document.querySelectorAll('[role="button"]'),
    ];
    const seen = new Set();
    const buttons = [];
    for (const el of buttonEls) {
        if (seen.has(el)) continue;
        seen.add(el);
        buttons.push({
            backend_node_id: 0,
            text: (el.textContent || el.value || '').trim().substring(0, 100),
            button_type: el.type || '',
            aria_label: el.getAttribute('aria-label') || '',
            is_visible: isVisible(el),
            form_index: getFormIndex(el),
            ax_role: el.getAttribute('role') || 'button',
            ax_name: '',
        });
    }

    // Collect links
    const links = Array.from(document.querySelectorAll('a[href]')).map(el => ({
        backend_node_id: 0,
        text: (el.textContent || '').trim().substring(0, 100),
        href: el.href || '',
        aria_label: el.getAttribute('aria-label') || '',
        is_visible: isVisible(el),
        in_nav: inNav(el),
        ax_role: el.getAttribute('role') || 'link',
        ax_name: '',
    }));

    return JSON.stringify({
        url: window.location.href,
        title: document.title || '',
        forms,
        inputs,
        buttons,
        links,
    });
})()
"#;

/// Raw JSON structure returned by the injected JS.
#[derive(Deserialize)]
struct RawPageData {
    url: String,
    title: String,
    forms: Vec<FormInfo>,
    inputs: Vec<InputInfo>,
    buttons: Vec<ButtonInfo>,
    links: Vec<LinkInfo>,
}

/// Analyze the current page and produce a PageSnapshot.
pub async fn analyze_page(
    page: &Page,
    logger: &SmartLoginLogger,
) -> crate::Result<PageSnapshot> {
    logger.log(LoginState::AnalyzingPage, "Analyzing page structure...");

    // Execute the analysis script in the page context
    let result = page
        .evaluate(PAGE_ANALYSIS_JS)
        .await
        .map_err(|e| crate::error::VaultError::SmartLoginError(
            format!("Page analysis script failed: {e}"),
        ))?;

    let json_str: String = result.into_value().map_err(|e| {
        crate::error::VaultError::SmartLoginError(format!("Failed to parse analysis result: {e}"))
    })?;

    let raw: RawPageData = serde_json::from_str(&json_str).map_err(|e| {
        crate::error::VaultError::SmartLoginError(format!("Invalid page data JSON: {e}"))
    })?;

    // Filter to only visible, non-readonly inputs for classification
    let visible_inputs: Vec<InputInfo> = raw
        .inputs
        .into_iter()
        .filter(|i| {
            i.is_visible
                && !i.is_readonly
                && !matches!(
                    i.input_type.as_str(),
                    "hidden" | "checkbox" | "radio" | "file" | "image" | "reset" | "color" | "range"
                )
        })
        .collect();

    let visible_buttons: Vec<ButtonInfo> = raw
        .buttons
        .into_iter()
        .filter(|b| b.is_visible)
        .collect();

    let visible_links: Vec<LinkInfo> = raw
        .links
        .into_iter()
        .filter(|l| l.is_visible && !l.text.is_empty())
        .collect();

    let snapshot = PageSnapshot {
        url: raw.url,
        title: raw.title,
        forms: raw.forms,
        inputs: visible_inputs,
        buttons: visible_buttons,
        links: visible_links,
    };

    logger.emit(
        SmartLoginEvent::new(
            LoginState::AnalyzingPage,
            format!(
                "Page analysis complete: {} forms, {} inputs, {} buttons, {} links",
                snapshot.forms.len(),
                snapshot.inputs.len(),
                snapshot.buttons.len(),
                snapshot.links.len(),
            ),
        )
        .with_detail(SmartLoginEventDetail::PageAnalysis {
            forms_found: snapshot.forms.len(),
            inputs_found: snapshot.inputs.len(),
            buttons_found: snapshot.buttons.len(),
            links_found: snapshot.links.len(),
        }),
    );

    Ok(snapshot)
}

/// Wait for DOM to stabilize after an interaction.
/// Uses readyState polling + a short MutationObserver.
/// Non-fatal — if the page navigates mid-wait, silently succeeds.
pub async fn wait_for_dom_settle(page: &Page, settle_ms: u64) -> crate::Result<()> {
    let js = format!(
        r#"
        new Promise((resolve) => {{
            try {{
                // If page is still loading, wait for readyState first
                if (document.readyState === 'loading') {{
                    document.addEventListener('DOMContentLoaded', () => resolve('loaded'), {{ once: true }});
                    setTimeout(() => resolve('timeout'), {settle_ms} * 2);
                    return;
                }}

                let timer = null;
                const observer = new MutationObserver(() => {{
                    if (timer) clearTimeout(timer);
                    timer = setTimeout(() => {{
                        observer.disconnect();
                        resolve('settled');
                    }}, Math.min({settle_ms}, 500));
                }});

                if (document.body) {{
                    observer.observe(document.body, {{
                        childList: true, subtree: true, attributes: false
                    }});
                }}

                // Resolve immediately if no mutations within settle time
                timer = setTimeout(() => {{
                    observer.disconnect();
                    resolve('quiet');
                }}, {settle_ms});

                // Hard fallback
                setTimeout(() => {{
                    observer.disconnect();
                    resolve('fallback');
                }}, {settle_ms} * 2);
            }} catch(e) {{
                resolve('error');
            }}
        }})
        "#
    );

    // Ignore errors — page may have navigated away (context destroyed)
    let _ = page.evaluate(js).await;

    Ok(())
}

/// Wait until the page has interactive content (inputs or buttons).
/// Polls every 300ms for up to `max_wait_ms`. Handles redirect chains
/// and JS-rendered login forms like Google, Microsoft, etc.
pub async fn wait_for_page_ready(page: &Page, max_wait_ms: u64) -> bool {
    const POLL_JS: &str = r#"
    (() => {
        const inputs = document.querySelectorAll('input:not([type="hidden"])');
        const buttons = document.querySelectorAll('button, [role="button"], input[type="submit"]');
        return JSON.stringify({
            ready: document.readyState,
            inputs: inputs.length,
            buttons: buttons.length,
            url: window.location.href,
        });
    })()
    "#;

    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_millis(max_wait_ms);
    let interval = tokio::time::Duration::from_millis(300);

    loop {
        if tokio::time::Instant::now() > deadline {
            return false;
        }

        if let Ok(result) = page.evaluate(POLL_JS).await
            && let Ok(json_str) = result.into_value::<String>()
                && let Ok(state) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    let inputs = state["inputs"].as_u64().unwrap_or(0);
                    let buttons = state["buttons"].as_u64().unwrap_or(0);
                    let ready = state["ready"].as_str().unwrap_or("");

                    // Page has content and is loaded
                    if (inputs > 0 || buttons > 1) && ready != "loading" {
                        return true;
                    }
                }

        tokio::time::sleep(interval).await;
    }
}

/// Get a CDP-compatible selector for a specific input by its attributes.
/// Falls back through id → name → type+placeholder → nth-of-type.
pub fn build_selector(input: &InputInfo) -> String {
    if !input.id.is_empty() {
        return format!("#{}", css_escape(&input.id));
    }
    if !input.name.is_empty() {
        return format!(
            "input[name=\"{}\"]",
            input.name.replace('"', r#"\""#)
        );
    }
    if !input.placeholder.is_empty() {
        return format!(
            "input[placeholder=\"{}\"]",
            input.placeholder.replace('"', r#"\""#)
        );
    }
    format!("input[type=\"{}\"]", input.input_type)
}

/// Build a CSS selector for a button.
pub fn build_button_selector(button: &ButtonInfo) -> String {
    if !button.aria_label.is_empty() {
        return format!(
            "[aria-label=\"{}\"]",
            button.aria_label.replace('"', r#"\""#)
        );
    }
    // Use XPath-like text match via JS instead
    "button, input[type=\"submit\"], [role=\"button\"]".to_string()
}

/// Minimal CSS escape for IDs
fn css_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '.' | ':' | '[' | ']' | '(' | ')' | '#' | '>' | '+' | '~' | ',' | ' ' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}
