//! Opt-in desktop diagnostics: local fixtures use dummy data; live Google tests
//! require an explicitly supplied test account and a separate hidden password opt-in.
//! Opens a visible window; never runs as part of the normal test suite.
use super::*;
use chromiumoxide::browser::{Browser, BrowserConfig};
use futures::StreamExt;

#[tokio::test]
#[ignore = "runs CDP session/discovery regressions in a disposable headless Brave on loopback"]
async fn cdp_session_browser_regression() {
    use futures::FutureExt;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local = format!("http://{}/", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        while let Ok((mut stream, _)) = listener.accept().await {
            let mut request = [0u8; 4096];
            let _ = stream.read(&mut request).await;
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 15\r\nConnection: close\r\n\r\n<!doctype html>").await;
            let _ = stream.shutdown().await;
        }
    });
    let profile = tempfile::tempdir().unwrap();
    let executable = std::env::var("YNTRA_TEST_BROWSER").unwrap_or_else(|_| r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe".into());
    let config = BrowserConfig::builder().chrome_executable(executable).user_data_dir(profile.path()).build().unwrap();
    let (mut browser, mut handler) = Browser::launch(config).await.unwrap();
    let task = tokio::spawn(async move { while let Some(event) = handler.next().await { if event.is_err() { break; } } });
    let result = std::panic::AssertUnwindSafe(async {
        let page = browser.new_page(&local).await.unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let logger = SmartLoginLogger::new(Box::new(|_| {}));
        let engine = SmartLoginEngine::new(SmartLoginConfig::default(), logger, cancel.clone());
        page.set_content("<header><button aria-label='demo@example.test' aria-controls='menu'>Account</button><div id='menu' hidden><a href='/logout'>Exit</a></div></header>").await.unwrap();
        assert!(matches!(engine.find_login_form(page.clone(), &local, "demo@example.test").await, Err(LoginResult::AlreadySignedIn { .. })));
        assert!(matches!(engine.find_login_form(page.clone(), &local, "unknown-username").await, Err(LoginResult::RequiresManualAction)));
        assert!(matches!(engine.find_login_form(page.clone(), &local, "other@example.test").await, Err(LoginResult::DifferentAccount)));
        assert_eq!(page.url().await.unwrap().unwrap(), local);
        page.set_content("<p>Two-factor verification code</p><input autocomplete='one-time-code'>").await.unwrap();
        page.evaluate("setTimeout(() => {document.body.innerHTML='<p>Loading...</p>'}, 450); setTimeout(() => {document.body.innerHTML='<header><button aria-controls=account>Profile</button><div id=account hidden><a href=/logout>Sign out</a></div></header>'}, 900)").await.unwrap();
        assert!(matches!(verifier::verify_login_result(&page, &local, &local, "demo@example.test", true, &cancel, &engine.logger).await.unwrap(), LoginResult::Success { .. }));
        page.set_content("<header><button aria-label='other@example.test'>Profile</button><a href='/logout'>Exit</a></header>").await.unwrap();
        assert!(matches!(verifier::verify_login_result(&page, &local, &local, "demo@example.test", false, &cancel, &engine.logger).await.unwrap(), LoginResult::DifferentAccount));
        cancel.store(true, Ordering::SeqCst);
        assert!(matches!(verifier::verify_login_result(&page, &local, &local, "demo@example.test", false, &cancel, &engine.logger).await.unwrap(), LoginResult::Cancelled));
    }).catch_unwind().await;
    let _ = browser.close().await;
    task.abort();
    server.abort();
    if let Err(panic) = result { std::panic::resume_unwind(panic); }
}

/// Read only the already-open GitHub page. Never launch/close/navigate the user's browser.
#[tokio::test]
#[ignore = "reads authentication UI metadata from an existing local Brave CDP session"]
async fn github_result_readonly_diagnostic() {
    let browser_info = browser::discover_browsers().into_iter()
        .find(|b| b.process_name.eq_ignore_ascii_case("brave.exe")).expect("Brave required");
    let endpoint = std::fs::read_to_string(browser_info.profile_dir.join("DevToolsActivePort")).expect("existing CDP session required");
    let mut lines = endpoint.lines();
    let port: u16 = lines.next().unwrap().parse().unwrap();
    let path = lines.next().unwrap();
    assert!(path.starts_with("/devtools/browser/"));
    let (browser, mut handler) = Browser::connect(format!("ws://127.0.0.1:{port}{path}")).await.unwrap();
    let task = tokio::spawn(async move { while let Some(event) = handler.next().await { if event.is_err() { break; } } });
    let mut found = false;
    for page in browser.pages().await.unwrap() {
        let url = page.url().await.unwrap_or_default().unwrap_or_default();
        if !reqwest::Url::parse(&url).is_ok_and(|u| u.scheme() == "https" && u.host_str() == Some("github.com")) { continue; }
        found = true;
        let facts = page.evaluate(r#"(() => {
            const visible = el => el.getClientRects().length > 0 && (!el.checkVisibility || el.checkVisibility({checkOpacity:true,checkVisibilityCSS:true}));
            return JSON.stringify({
                loggedInBody: document.body.classList.contains('logged-in'),
                userLoginPresent: !!document.querySelector('meta[name="user-login"]')?.content,
                visibleAccountControls: [...document.querySelectorAll('header button, [role="banner"] button')].filter(e => visible(e) && (e.querySelector('img.avatar') || e.getAttribute('aria-haspopup'))).map(e => ({ tag:e.tagName, hasAvatar:!!e.querySelector('img.avatar'), popup:e.getAttribute('aria-haspopup'), controls:e.getAttribute('aria-controls'), target:e.getAttribute('data-target')})),
                logoutForms: [...document.querySelectorAll('form[action]')].filter(e => /\/(logout|signout)\b/.test(e.getAttribute('action'))).map(e => ({visible:visible(e), insideMenu:!!e.closest('[role="menu"],dialog,details'), method:e.method})),
                visiblePasswords: [...document.querySelectorAll('input[type="password"]')].filter(visible).length,
                ready: document.readyState
            });
        })()"#).await.unwrap().into_value::<String>().unwrap();
        eprintln!("GitHub authentication structure (no account values): {facts}");
    }
    task.abort();
    assert!(found, "No existing GitHub tab found");
}

/// Exercise the integrated ordinary-browser route; leave the window open for inspection.
/// Credentials are supplied at runtime, never through test source or command-line passwords.
#[tokio::test]
#[ignore = "opens ordinary Brave on Google; password input requires a separate explicit opt-in and hidden terminal prompt"]
async fn google_native_smart_login_diagnostic() {
    let text = Zeroizing::new(std::env::var("YNTRA_GOOGLE_TEST_EMAIL").expect("explicit test identifier required"));
    let full_login = std::env::var("YNTRA_GOOGLE_TEST_PASSWORD_PROMPT").as_deref() == Ok("1");
    let password = if full_login {
        eprintln!("Enter the authorized test account password at the hidden terminal prompt:");
        Zeroizing::new(rpassword::read_password().expect("hidden password prompt unavailable"))
    } else { Zeroizing::new(String::new()) };
    if full_login { assert!(!password.is_empty(), "empty password is not a full-login test"); }
    let browser = browser::discover_browsers().into_iter()
        .find(|b| b.process_name.eq_ignore_ascii_case("brave.exe")).expect("Brave required");
    let password_step = Arc::new(AtomicBool::new(false));
    let typed_identifier = Arc::new(AtomicBool::new(false));
    let observed_typing = typed_identifier.clone();
    let observed_password = password_step.clone();
    let started = std::time::Instant::now();
    let logger = SmartLoginLogger::new(Box::new(move |event| {
        if event.message == "Typing identifier with the system keyboard" {
            observed_typing.store(true, Ordering::SeqCst);
        }
        if event.message == "Visible password field reached; no password supplied, stopping here" {
            observed_password.store(true, Ordering::SeqCst);
        }
        eprintln!("{}ms {:?}: {}", started.elapsed().as_millis(), event.state, event.message);
    }));
    let engine = SmartLoginEngine::new(SmartLoginConfig::default(), logger, Arc::new(AtomicBool::new(false)));
    let result = engine.execute("https://gmail.com", text, password, None, &browser).await;
    eprintln!("Integrated result: {result:?}; browser intentionally left open");
    if std::env::var("YNTRA_GOOGLE_TEST_BLANK_LOGIN").as_deref() == Ok("1") {
        assert!(typed_identifier.load(Ordering::SeqCst), "Blank-form regression must actually type the identifier");
    }
    assert!((full_login && matches!(result, LoginResult::Success { .. }))
        || matches!(result, LoginResult::AlreadySignedIn { .. } | LoginResult::RequiresCaptcha | LoginResult::RequiresMfa { .. })
        || (matches!(result, LoginResult::RequiresManualAction) && password_step.load(Ordering::SeqCst)),
        "Expected the selected session, a password step or verification, got {result:?}");
}

/// Opt-in investigation against Google's identifier step. Never supplies a password.
/// The account identifier comes from the caller, not from source or a vault.
#[tokio::test]
#[ignore = "contacts Google with an explicitly supplied test identifier; never run unattended"]
async fn google_identifier_diagnostic() {
    let text = Zeroizing::new(std::env::var("YNTRA_GOOGLE_TEST_EMAIL").expect("explicit test identifier required"));
    let mode = std::env::var("YNTRA_GOOGLE_TEST_MODE").unwrap_or_else(|_| "smart".into());
    assert!(matches!(mode.as_str(), "smart" | "auto" | "enter" | "manual"));
    let profile = tempfile::tempdir().unwrap();
    let config = BrowserConfig::builder()
        .chrome_executable(r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe")
        .user_data_dir(profile.path()).with_head().viewport(None).disable_default_args()
        .arg("--no-first-run").build().unwrap();
    let (mut browser, mut handler) = Browser::launch(config).await.unwrap();
    let handler_task = tokio::spawn(async move { while let Some(event) = handler.next().await { if event.is_err() { break; } } });
    use futures::FutureExt;
    let outcome = std::panic::AssertUnwindSafe(async {
        let page = browser.new_page("https://accounts.google.com/AddSession?service=mail").await.unwrap();
        page.bring_to_front().await.unwrap();
        let mut last_initial = serde_json::Value::Null;
        let initial = tokio::time::timeout(std::time::Duration::from_secs(25), async {
            loop {
                let result = page.evaluate(r#"(() => ({
                    host: location.hostname, path: location.pathname, title: document.title, documentReady:document.readyState,
                    ready: [...document.querySelectorAll('input')].some(e => e.getClientRects().length && ['email','text','tel'].includes(e.type)),
                    inputs: [...document.querySelectorAll('input')].map(e => ({id:e.id,type:e.type,autocomplete:e.autocomplete,visible:!!e.getClientRects().length})),
                    text: document.body?.innerText.slice(0,1500)
                }))()"#).await;
                if let Ok(result) = result {
                    let state = result.into_value::<serde_json::Value>().unwrap();
                    if state["ready"] == true { break state; }
                    last_initial = state;
                }
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        }).await.unwrap_or_else(|_| panic!("Google identifier page did not load: {last_initial}"));
        assert_eq!(initial["host"], "accounts.google.com");
        eprintln!("Initial Google page: {initial}");
        page.bring_to_front().await.unwrap();
        let field = page.find_element("input:not([type=hidden])").await.unwrap();
        field.click().await.unwrap();
        // Count event categories only: no keystrokes, cookies, tokens or network bodies.
        page.evaluate(r#"window.yntraDiagnostic = {input:0,untrustedInput:0,paste:0,keydown:0,unidentified:0};
            for (const type of ['input','paste','keydown']) document.addEventListener(type, e => {
                window.yntraDiagnostic[type]++;
                if (e.type === 'input' && !e.isTrusted) window.yntraDiagnostic.untrustedInput++;
                if (e.type === 'keydown' && e.key === 'Unidentified') window.yntraDiagnostic.unidentified++;
            });"#).await.unwrap();
        if mode == "manual" {
            eprintln!("MANUAL_CONTROL_READY: type the supplied test identifier physically, click Next, and stop before entering any password.");
        } else if mode != "auto" {
            page.bring_to_front().await.unwrap();
            let engine = SmartLoginEngine::new(SmartLoginConfig::default(), SmartLoginLogger::new(Box::new(|_| {})), Arc::new(AtomicBool::new(false)));
            engine.direct_fill_field(&page, "identifier", &text).await.unwrap();
            eprintln!("Identifier event counts: {}", page.evaluate("window.yntraDiagnostic").await.unwrap().into_value::<serde_json::Value>().unwrap());
            if mode == "enter" { page.find_element("input:focus").await.unwrap().press_key("Enter").await.unwrap(); }
            else { engine.direct_submit(&page).await; }
        } else {
            crate::services::autotype::run_smart_autotype_with_delays(text.to_string(), String::new(), String::new(), "https://accounts.google.com/".into(), false, 15, 300).unwrap();
        }
        let result = tokio::time::timeout(std::time::Duration::from_secs(if mode == "manual" { 180 } else { 20 }), async {
            loop {
                if let Ok(result) = page.evaluate(r#"(() => {
                    const text = document.body?.innerText || '';
                    const password = [...document.querySelectorAll('input[type=password]')].some(e => e.getClientRects().length);
                    const rejected = /browser or app may not be secure|webbläsaren eller appen kanske inte är säker|couldn.t sign you in|inloggningen misslyckades/i.test(text);
                    return {password,rejected,path:location.pathname,text:text.slice(0,1800),events:window.yntraDiagnostic};
                })()"#).await {
                    let state = result.into_value::<serde_json::Value>().unwrap();
                    if state["password"] == true || state["rejected"] == true { break state; }
                }
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
        }).await;
        match result {
            Ok(state) => eprintln!("Google identifier outcome ({mode}): {state}"),
            Err(_) => eprintln!("Google identifier outcome ({mode}): no terminal result; {}", page.evaluate("JSON.stringify({text:document.body.innerText.slice(0,1800),events:window.yntraDiagnostic,length:document.querySelector('#identifierId')?.value.length})").await.unwrap().into_value::<String>().unwrap()),
        }
    }).catch_unwind().await;
    let _ = browser.close().await;
    handler_task.abort();
    if let Err(panic) = outcome { std::panic::resume_unwind(panic); }
}

#[tokio::test]
#[ignore = "opens a disposable Brave window and sends keyboard input to a local test page"]
async fn windows_keyboard_browser_smoke() {
    // A real local URL also exercises the OS address-bar/CDP target binding.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_url = format!("http://{}/", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        while let Ok((mut stream, _)) = listener.accept().await {
            let mut request = [0u8; 4096];
            let _ = stream.read(&mut request).await;
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 15\r\nConnection: close\r\n\r\n<!doctype html>").await;
            let _ = stream.shutdown().await;
        }
    });
    let profile = tempfile::tempdir().unwrap();
    let executable = std::env::var("YNTRA_TEST_BROWSER").unwrap_or_else(|_| {
        r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe".into()
    });
    let config = BrowserConfig::builder()
        .chrome_executable(executable)
        .user_data_dir(profile.path())
        .with_head()
        .viewport(None)
        .disable_default_args()
        .arg("--no-first-run")
        .build()
        .unwrap();
    let (mut browser, mut handler) = Browser::launch(config).await.unwrap();
    let handler_task = tokio::spawn(async move {
        while let Some(event) = handler.next().await {
            if event.is_err() {
                break;
            }
        }
    });
    // Catch test assertions so the disposable browser is closed even on failure.
    use futures::FutureExt;
    let outcome = std::panic::AssertUnwindSafe(async {
        let page = browser.new_page(local_url).await.unwrap();
        page.set_content(r#"<!doctype html><title>Yntra keyboard test</title>
            <label>Email <input id="identifierId" type="email" autocomplete="username"></label>
            <input id="trap" aria-label="Unrelated field">
            <script>
            window.events = [];
            for (const type of ['keydown','keyup','input','paste']) {
                identifierId.addEventListener(type, e => events.push({type:e.type,key:e.key,code:e.code,trusted:e.isTrusted,time:e.timeStamp}));
            }
            </script>"#).await.unwrap();
        page.bring_to_front().await.unwrap();
        page.find_element("#identifierId").await.unwrap().click().await.unwrap();
        // Windows can refuse programmatic activation while the user operates another window.
        // Wait for this disposable test window; do not send keys into that other window.
        let focus_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(25);
        loop {
            let mut title = [0u16; 256];
            let len = unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowTextW(
                windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow(), &mut title) };
            if String::from_utf16_lossy(&title[..len.max(0) as usize]).starts_with("Yntra keyboard test") { break; }
            assert!(tokio::time::Instant::now() < focus_deadline, "Activate the disposable Yntra keyboard test window");
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        let engine = SmartLoginEngine::new(SmartLoginConfig::default(), SmartLoginLogger::new(Box::new(|_| {})), cancelled.clone());
        let text = Zeroizing::new("Demo.+_%@example.test".to_string());
        engine.direct_fill_field(&page, "identifier", &text).await.unwrap();
        let value = page.evaluate("document.getElementById('identifierId').value").await.unwrap().into_value::<String>().unwrap();
        assert_eq!(value, *text);
        let events = page.evaluate("window.events").await.unwrap().into_value::<Vec<serde_json::Value>>().unwrap();
        assert!(!events.iter().any(|e| e["type"] == "paste"));
        assert!(events.iter().filter(|e| e["type"] == "input").all(|e| e["trusted"] == true));
        let down = events.iter().find(|e| e["type"] == "keydown" && e["key"] == "D").expect("native uppercase keydown");
        let up = events.iter().find(|e| e["type"] == "keyup" && e["key"] == "D").expect("native uppercase keyup");
        assert_eq!(down["code"], "KeyD");
        assert!(up["time"].as_f64().unwrap() > down["time"].as_f64().unwrap());

        // Preserve native characters AND unmapped/astral Unicode without clipboard use.
        let unicode = Zeroizing::new("åÅé漢😀+_%@example.test".to_string());
        engine.direct_fill_field(&page, "identifier", &unicode).await.unwrap();
        assert_eq!(page.evaluate("document.getElementById('identifierId').value").await.unwrap().into_value::<String>().unwrap(), *unicode);
        assert!(!page.evaluate("window.events.some(e => e.type === 'paste')").await.unwrap().into_value::<bool>().unwrap());

        let control = Zeroizing::new("name\tother@example.test".to_string());
        assert!(engine.direct_fill_field(&page, "identifier", &control).await.is_err());
        assert_eq!(page.evaluate("document.getElementById('identifierId').value").await.unwrap().into_value::<String>().unwrap(), *unicode);

        page.evaluate("document.getElementById('identifierId').value = ''; window.events = []; document.getElementById('identifierId').readOnly = true").await.unwrap();
        assert!(engine.direct_fill_field(&page, "identifier", &text).await.is_err());
        assert_eq!(page.evaluate("window.events.length").await.unwrap().into_value::<usize>().unwrap(), 0);

        // A page that moves focus after the first character must not receive the rest elsewhere.
        page.evaluate("document.getElementById('identifierId').readOnly = false; document.getElementById('identifierId').addEventListener('input', () => document.getElementById('trap').focus(), {once:true})").await.unwrap();
        assert!(engine.direct_fill_field(&page, "identifier", &text).await.is_err());
        assert_eq!(page.evaluate("document.getElementById('trap').value").await.unwrap().into_value::<String>().unwrap(), "");

        // Rejected input must fail verification, with no submission/retry.
        page.evaluate("document.getElementById('identifierId').value = ''; document.getElementById('identifierId').addEventListener('beforeinput', e => e.preventDefault())").await.unwrap();
        assert!(engine.direct_fill_field(&page, "identifier", &text).await.is_err());
        assert_eq!(page.evaluate("document.getElementById('identifierId').value").await.unwrap().into_value::<String>().unwrap(), "");
        // A CSS-hidden decoy must not redirect a password into the identifier.
        page.set_content(r#"<input id="identifierId" autocomplete="username webauthn"><input type="password" style="display:none"><input id="trap">"#).await.unwrap();
        page.find_element("#identifierId").await.unwrap().click().await.unwrap();
        let dummy_password = Zeroizing::new("Dummy-Secret-123!".into());
        assert!(engine.direct_fill_field(&page, "password", &dummy_password).await.is_err());
        assert_eq!(page.evaluate("identifierId.value").await.unwrap().into_value::<String>().unwrap(), "");
        page.evaluate("document.querySelector('input[type=password]').style.display = ''; document.querySelector('input[type=password]').id='passwordId'").await.unwrap();
        engine.direct_fill_field(&page, "password", &dummy_password).await.unwrap();
        assert_eq!(page.evaluate("passwordId.value").await.unwrap().into_value::<String>().unwrap(), *dummy_password);
        page.evaluate("passwordId.value=''; passwordId.addEventListener('input',()=>trap.focus(),{once:true})").await.unwrap();
        assert!(engine.direct_fill_field(&page, "password", &dummy_password).await.is_err());
        assert_eq!(page.evaluate("trap.value").await.unwrap().into_value::<String>().unwrap(), "");
        cancelled.store(true, Ordering::SeqCst);
        assert!(engine.direct_fill_field(&page, "identifier", &text).await.is_err());

        // Exercise the real Windows accessibility observer on local fixtures only.
        use crate::smartlogin::native_state::PageState;
        for (html, address, expected) in [
            ("<input id='identifierId'><input type='password' style='display:none'>", "https://accounts.google.com/signin", PageState::Identifier),
            ("<input type='password' aria-invalid='true'>", "https://accounts.google.com/signin", PageState::InvalidCredentials),
            ("<label><input type='checkbox'>I'm not a robot</label>", "https://accounts.google.com/signin", PageState::Captcha),
            ("<input id='totpPin'>", "https://accounts.google.com/signin", PageState::Mfa),
            ("<button aria-label='Google Account: Demo (demo@example.test)'>Account</button>", "https://mail.google.com/mail/u/0/", PageState::Authenticated),
            ("<button aria-label='Google Account: Other (other@example.test)'>Account</button>", "https://mail.google.com/mail/u/0/", PageState::OtherAccount),
            ("<p>Loading...</p>", "https://mail.google.com/mail/u/0/", PageState::Unknown),
        ] {
            page.set_content(html).await.unwrap();
            page.bring_to_front().await.unwrap();
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(3);
            loop {
                let observed = tokio::task::spawn_blocking(move || crate::services::autotype::observe_test_window(address, "demo@example.test")).await.unwrap();
                if observed == expected { break; }
                assert!(tokio::time::Instant::now() < deadline, "Expected {expected:?}, got {observed:?}");
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
        // Generic CDP detection uses semantics and an exact identity, not a provider name or English text.
        page.set_content("<header><button aria-haspopup='menu' aria-label='demo@example.test'>Profile</button><a href='/logout'>退出</a></header>").await.unwrap();
        let local = page.url().await.unwrap().unwrap();
        assert!(matches!(verifier::existing_session(&page, &local, "demo@example.test").await, Some(LoginResult::AlreadySignedIn { .. })));
        assert!(matches!(verifier::existing_session(&page, &local, "other@example.test").await, Some(LoginResult::DifferentAccount)));
        cancelled.store(false, Ordering::SeqCst);
        assert!(matches!(engine.find_login_form(page.clone(), &local, "demo@example.test").await, Err(LoginResult::AlreadySignedIn { .. })));
        assert_eq!(page.url().await.unwrap().unwrap(), local);
        assert!(matches!(engine.find_login_form(page.clone(), &local, "unknown-username").await, Err(LoginResult::RequiresManualAction)));
        assert_eq!(page.url().await.unwrap().unwrap(), local);
        page.evaluate("document.body.insertAdjacentHTML('beforeend', '<input type=password>')").await.unwrap();
        assert!(verifier::existing_session(&page, &local, "demo@example.test").await.is_none());

        // Submitted TOTP can remain on screen, then disappear during a delayed transition.
        page.set_content("<p>Two-factor verification code</p><input autocomplete='one-time-code'>").await.unwrap();
        page.evaluate("setTimeout(() => {document.body.innerHTML='<p>Loading...</p>'}, 450); setTimeout(() => {document.body.innerHTML='<header><button aria-controls=account>Profile</button><div id=account hidden><a href=/logout>Sign out</a></div></header>'}, 900)").await.unwrap();
        assert!(matches!(verifier::verify_login_result(&page, &local, &local, "demo@example.test", true, &cancelled, &engine.logger).await.unwrap(), LoginResult::Success { .. }));
        cancelled.store(true, Ordering::SeqCst);
        assert!(matches!(verifier::verify_login_result(&page, &local, &local, "demo@example.test", false, &cancelled, &engine.logger).await.unwrap(), LoginResult::Cancelled));
    }).catch_unwind().await;
    let _ = browser.close().await;
    handler_task.abort();
    server.abort();
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
}
