import { describe, it, expect } from 'bun:test';
import { smartLoginResultView } from './smartLoginResult';

describe('Smart Login result messages', () => {
  it('distinguishes existing and newly authenticated sessions', () => {
    expect(smartLoginResultView({ Success: { final_url: 'https://mail.google.com/mail/' } }).key).toBe('smart_login.result_success');
    expect(smartLoginResultView({ AlreadySignedIn: { final_url: 'https://mail.google.com/mail/' } }).key).toBe('smart_login.result_already');
  });
  it('never renders missing or unknown results as success', () => {
    for (const result of [null, undefined, {}, 'NewUnknownResult', { UnexpectedState: { description: 'loading' } }]) {
      expect(smartLoginResultView(result).tone).toBe('action');
    }
  });
  it('shows specific errors and user actions without exposing backend payloads', () => {
    expect(smartLoginResultView({ WrongCredentials: { error_message: 'private page text' } }).key).toBe('smart_login.result_credentials');
    expect(smartLoginResultView('DifferentAccount').tone).toBe('action');
    expect(smartLoginResultView('RequiresCaptcha').key).toBe('smart_login.result_captcha');
    expect(smartLoginResultView({ RequiresMfa: { mfa_type: 'OTP' } }).key).toBe('smart_login.result_mfa');
  });
});
