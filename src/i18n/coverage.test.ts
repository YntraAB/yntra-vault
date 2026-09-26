import { describe, expect, it } from 'bun:test';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';
import { LANGUAGES } from './languages';
import { en } from './locales/en';
import { getTranslation, loadTranslation } from './translations';

const root = fileURLToPath(new URL('../../', import.meta.url));
const reference: Record<string, string> = en;
const keys = Object.keys(reference).sort();
const placeholders = (value: string) => [...new Set(
  [...value.matchAll(/\{\s*(\w+)\s*\}/g)].map(match => match[1]),
)].sort();

describe('translation coverage', () => {
  for (const { code } of LANGUAGES) {
    it(`${code} has the complete catalog, matching parameters and no duplicate keys`, async () => {
      const module = await import(`./locales/${code}.ts`);
      const dict: Record<string, string> = module.default ?? Object.values(module)[0];
      expect(Object.keys(dict).sort()).toEqual(keys);
      // Also check registry coverage; unknown language codes otherwise silently fall back.
      expect(await loadTranslation(code)).toBe(dict);
      const issues: string[] = [];
      for (const key of keys) {
        // Turkish puts the entire question after the object name.
        const intentionalEmpty = code === 'tr' && key === 'delete.confirm_before';
        if (typeof dict[key] !== 'string' || (!intentionalEmpty && !dict[key].trim())) issues.push(`${key}: empty`);
        if (JSON.stringify(placeholders(dict[key])) !== JSON.stringify(placeholders(reference[key]))) issues.push(`${key}: parameters`);
      }
      const file = new URL(`./locales/${code}.ts`, import.meta.url);
      const source = ts.createSourceFile(file.pathname, readFileSync(file, 'utf8'), ts.ScriptTarget.Latest, true);
      const seen = new Set<string>();
      const walk = (node: ts.Node) => {
        if (ts.isPropertyAssignment(node) && ts.isStringLiteral(node.name)) {
          if (seen.has(node.name.text)) issues.push(`${node.name.text}: duplicate`);
          seen.add(node.name.text);
        }
        ts.forEachChild(node, walk);
      };
      walk(source);
      expect(issues).toEqual([]);
      expect(getTranslation(code, 'app.search_placeholder', { shortcut: 'Alt+Shift+F' })).toContain('Alt+Shift+F');
    });
  }

  it('resolves literal UI keys and keys from label maps', () => {
    const issues: string[] = [];
    const check = (key: string, file: string) => {
      if (!Object.prototype.hasOwnProperty.call(reference, key)) issues.push(`${file}: ${key}`);
    };
    for (const rawFile of new Bun.Glob('src/**/*.{ts,tsx}').scanSync(root)) {
      const file = rawFile.replaceAll('\\', '/');
      if (file.includes('/i18n/locales/') || file.includes('.test.')) continue;
      const source = ts.createSourceFile(file, readFileSync(new URL(`../../${file}`, import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true);
      const walk = (node: ts.Node) => {
        if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) && ['t', 'getTranslation'].includes(node.expression.text)) {
          const arg = node.arguments[node.expression.text === 't' ? 0 : 1];
          if (arg && (ts.isStringLiteral(arg) || ts.isNoSubstitutionTemplateLiteral(arg))) check(arg.text, file);
        }
        if (ts.isPropertyAssignment(node) && ['labelKey', 'nameKey'].includes(node.name.getText(source)) && ts.isStringLiteral(node.initializer)) check(node.initializer.text, file);
        if (ts.isVariableDeclaration(node) && node.name.getText(source) === 'ordinalKeys' && node.initializer && ts.isArrayLiteralExpression(node.initializer)) {
          for (const element of node.initializer.elements) if (ts.isStringLiteral(element)) check(element.text, file);
        }
        ts.forEachChild(node, walk);
      };
      walk(source);
    }
    for (const level of ['Critical', 'Weak', 'Fair', 'Strong', 'Excellent']) check(`strength.${level.toLowerCase()}`, 'dynamic strength labels');
    expect(issues).toEqual([]);
  });

  it('preserves literal user values and intentional empty translations', async () => {
    await Promise.all(['en', 'tr'].map(loadTranslation));
    const value = "$& $$ $' $` {count}";
    expect(getTranslation('en', 'tags.delete_unused_count', { count: value })).toBe(`${value} unused tags`);
    expect(getTranslation('en', 'tags.delete_unused_count', { count: '{other}', other: 'changed' })).toBe('{other} unused tags');
    expect(getTranslation('tr', 'delete.confirm_before')).toBe('');
    expect(getTranslation('en', 'tags.delete_unused_count')).toBe('{count} unused tags');
    expect(getTranslation('en', 'missing.test.key')).toBe('missing.test.key');
  });
});
