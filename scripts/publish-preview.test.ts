import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { load } from 'js-yaml';
import { previewFiles, validatePreview, verifyExistingAsset } from './publish-preview.mjs';

test('preview packages have an exact platform allowlist', () => {
    expect(previewFiles('0.2.8', 'windows')).toHaveLength(4);
    expect(previewFiles('0.2.8', 'android')).toEqual(['Yntra.Vault_0.2.8_universal.apk']);
    expect(() => previewFiles('../secret', 'windows')).toThrow();
    expect(() => previewFiles('0.2.8', 'other')).toThrow();
});

test('published previews accept only the exact recorded source and stable releases are immutable', () => {
    const commit = 'a'.repeat(40);
    const source = { commit, tag: 'v0.2.8' };
    const release = { prerelease: true, draft: false, tag_name: 'v0.2.8' };
    expect(() => validatePreview(release, source, 'v0.2.8', commit)).not.toThrow();
    expect(() => validatePreview({ ...release, prerelease: false }, source, 'v0.2.8', commit)).toThrow();
    expect(() => validatePreview(release, source, 'v0.2.8', 'b'.repeat(40))).toThrow();
    expect(() => validatePreview(release, source, 'v0.2.9', commit)).toThrow();
});

test('existing binaries cannot be replaced, including on a retry', () => {
    const hash = 'a'.repeat(64);
    expect(() => verifyExistingAsset({ name: 'app.exe', digest: `sha256:${hash}` }, hash)).not.toThrow();
    expect(() => verifyExistingAsset({ name: 'app.exe', digest: 'sha256:different' }, hash)).toThrow();
    expect(() => verifyExistingAsset({ name: 'app.exe' }, hash)).toThrow();
});

test('Windows public preview is independent of other platforms and stable finalization is gated', () => {
    const workflow = load(readFileSync('.github/workflows/release.yml', 'utf8')) as {
        jobs: Record<string, { needs: string[]; if: string; steps: { env?: Record<string, string> }[] }>;
    };
    expect(workflow.jobs['preview-windows'].needs).toEqual(['prepare', 'build-windows']);
    expect(workflow.jobs['preview-linux'].needs).toContain('preview-windows');
    expect(workflow.jobs['preview-android'].needs).toContain('verify-android');
    expect(workflow.jobs.finalize.if).toContain("needs.prepare.outputs.preview != 'true'");
    expect(workflow.jobs['stage-packages'].if).toContain("needs.prepare.outputs.preview != 'true'");
});
