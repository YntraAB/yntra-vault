// Count Unicode scalar values like Rust; never trim or normalize the credential.
export function isValidNewMasterPassword(password: string): boolean {
  return password.trim().length > 0 && Array.from(password).length >= 12;
}
