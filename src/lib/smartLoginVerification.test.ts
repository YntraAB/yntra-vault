import { describe, expect, it } from 'bun:test';
import { readFileSync } from 'node:fs';
import { Window } from 'happy-dom';

const observer = readFileSync(new URL('../../crates/core/src/smartlogin/verify_page.js', import.meta.url), 'utf8');

function observe(html: string, expected = 'demo', url = 'https://github.com/') {
  const window = new Window({ url });
  window.document.write(html);
  // Happy DOM has no layout; model visibility explicitly. Real layout is covered
  // by the opt-in Brave test that runs this same production observer.
  for (const element of window.document.querySelectorAll('*')) {
    Object.defineProperty(element, 'getClientRects', {
      value() { return element.closest('[hidden], [style*="display:none"]') ? [] : [{}]; },
    });
    Object.defineProperty(element, 'checkVisibility', { value() { return true; } });
  }
  try {
    return JSON.parse(new Function('document', 'location', 'window', `return ${observer.replace('__EXPECTED_IDENTIFIER__', JSON.stringify(expected))}`)(window.document, window.location, window));
  } finally { window.happyDOM.abort(); }
}

describe('Production Smart Login page observer', () => {
  const github = '<head><meta name="user-login" content="demo"></head><body class="logged-in"><button>Account</button></body>';
  it('recognizes the authenticated GitHub viewer without an open logout menu', () => {
    expect(observe(github)).toMatchObject({ authenticated_control: true, identity_match: true, identity_mismatch: false });
    expect(observe(github, 'other')).toMatchObject({ authenticated_control: true, identity_match: false, identity_mismatch: true });
  });
  it('does not infer a GitHub username from an email address', () => {
    expect(observe(github, 'demo@example.test')).toMatchObject({ authenticated_control: true, identity_match: false, identity_mismatch: false });
  });
  it('rejects repository metadata, missing session markers and lookalike origins', () => {
    expect(observe('<meta name="octolytics-dimension-repository_owner" content="demo"><body class="logged-in">').authenticated_control).toBe(false);
    expect(observe('<meta name="user-login" content="demo"><body>').authenticated_control).toBe(false);
    expect(observe(github, 'demo', 'https://github.com.evil.test/').authenticated_control).toBe(false);
  });
  it('recognizes a logout action in a menu explicitly owned by a visible trigger', () => {
    expect(observe('<button aria-controls="menu">Account</button><div id="menu" hidden><form action="/logout"><button>Exit</button></form></div>').authenticated_control).toBe(true);
  });
  it('ignores unrelated hidden logout templates and external logout links', () => {
    expect(observe('<div hidden><a href="/logout">Sign out</a></div>').authenticated_control).toBe(false);
    expect(observe('<button aria-controls="menu">Account</button><div id="menu" hidden><a href="https://evil.test/logout">Sign out</a></div>').authenticated_control).toBe(false);
  });
  it('requires an unambiguous exact email identity', () => {
    const account = '<header><button aria-label="demo@example.test">Profile</button><a href="/logout">Exit</a></header>';
    expect(observe(account, 'demo@example.test').identity_match).toBe(true);
    expect(observe(account, 'other@example.test').identity_mismatch).toBe(true);
    expect(observe(account + '<button aria-haspopup="menu" aria-label="other@example.test">Other</button>', 'demo@example.test')).toMatchObject({ identity_match: false, identity_ambiguous: true });
  });
  it('keeps active challenges and credential fields alongside authenticated signals', () => {
    expect(observe(github + '<input type="password"><input autocomplete="one-time-code"><p>Two-factor verification code</p><div class="captcha">Check</div>')).toMatchObject({ has_password_input: true, has_mfa_input: true, has_mfa_text: true, has_captcha: true });
    expect(observe('<input type="password" hidden><div class="captcha" hidden>Check</div>')).toMatchObject({ has_password_input: false, has_captcha: false });
  });
});
