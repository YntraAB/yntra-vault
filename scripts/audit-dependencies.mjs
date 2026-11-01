import fs from 'node:fs';
import { pathToFileURL } from 'node:url';

export function classifyAdvisory(advisory) {
    if (advisory.withdrawn) return 'withdrawn';
    const affected = advisory.affected;
    if (!Array.isArray(affected) || !affected.length) throw new Error(`Incomplete advisory: ${advisory.id}`);
    return affected.every(entry => entry.database_specific?.informational === 'unmaintained') ? 'maintenance' : 'vulnerability';
}

async function jsonRequest(route, body) {
    for (let attempt = 0; attempt < 3; attempt++) {
        const response = await fetch(`https://api.osv.dev/v1/${route}`, {
            method: body ? 'POST' : 'GET',
            headers: { 'Content-Type': 'application/json' },
            body: body ? JSON.stringify(body) : undefined,
            signal: AbortSignal.timeout(30_000),
        });
        if (response.ok) return response.json();
        if (response.status !== 429 && response.status < 500) throw new Error(`Advisory lookup failed: HTTP ${response.status}`);
        await new Promise(resolve => setTimeout(resolve, 500 * (attempt + 1)));
    }
    throw new Error('Advisory service unavailable; audit did not complete');
}

export async function auditRustDependencies() {
    const packages = Bun.TOML.parse(fs.readFileSync('Cargo.lock', 'utf8')).package.filter(item => item.source?.startsWith('registry+'));
    const findings = [];
    for (let offset = 0; offset < packages.length; offset += 100) {
        const batch = packages.slice(offset, offset + 100);
        const response = await jsonRequest('querybatch', {
            queries: batch.map(item => ({ package: { name: item.name, ecosystem: 'crates.io' }, version: item.version })),
        });
        if (response.results?.length !== batch.length) throw new Error('Incomplete advisory batch response');
        response.results.forEach((result, index) => {
            for (const advisory of result.vulns ?? []) findings.push({ name: batch[index].name, version: batch[index].version, id: advisory.id });
        });
    }
    const details = new Map();
    for (const id of new Set(findings.map(item => item.id))) details.set(id, await jsonRequest(`vulns/${encodeURIComponent(id)}`));
    let vulnerabilities = 0;
    for (const finding of findings) {
        const advisory = details.get(finding.id);
        const kind = classifyAdvisory(advisory);
        if (kind === 'withdrawn') continue;
        if (kind === 'vulnerability') vulnerabilities++;
        console.log(`${kind}: ${finding.name} ${finding.version}: ${finding.id}: ${advisory.summary}`);
    }
    console.log(`Checked ${packages.length} Cargo registry packages. Local/path dependencies require separate source review.`);
    if (vulnerabilities) throw new Error(`${vulnerabilities} vulnerable Cargo dependency matches`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) await auditRustDependencies();
