export function smartLoginResultView(result: unknown) {
  const kind = typeof result === 'string' ? result
    : result && typeof result === 'object' ? Object.keys(result)[0] : '';
  const outcomes: Record<string, { key: string; tone: 'success' | 'action' | 'error' }> = {
    Success: { key: 'smart_login.result_success', tone: 'success' },
    AlreadySignedIn: { key: 'smart_login.result_already', tone: 'success' },
    DifferentAccount: { key: 'smart_login.result_other_account', tone: 'action' },
    RequiresCaptcha: { key: 'smart_login.result_captcha', tone: 'action' },
    RequiresMfa: { key: 'smart_login.result_mfa', tone: 'action' },
    RequiresManualAction: { key: 'smart_login.continue_in_browser', tone: 'action' },
    UnexpectedState: { key: 'smart_login.continue_in_browser', tone: 'action' },
    WrongCredentials: { key: 'smart_login.result_credentials', tone: 'error' },
    AccountLocked: { key: 'smart_login.result_locked', tone: 'error' },
    DomainMismatch: { key: 'smart_login.result_domain', tone: 'error' },
    Timeout: { key: 'smart_login.result_timeout', tone: 'action' },
    Cancelled: { key: 'smart_login.result_cancelled', tone: 'action' },
    BrowserError: { key: 'smart_login.result_browser', tone: 'error' },
    LoginFormNotFound: { key: 'smart_login.result_form', tone: 'action' },
  };
  return outcomes[kind ?? ''] ?? { key: 'smart_login.continue_in_browser', tone: 'action' as const };
}
