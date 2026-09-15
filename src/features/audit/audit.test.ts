import { describe, it, expect } from 'bun:test';
import { formatCount, getStatusConfig } from './components/BreachIndicator';
import { getLocalizedIssueDescription } from './components/SecurityDashboard';
import type { SecurityIssue } from '@/lib/backend';

describe('Audit Feature Slice', () => {
  describe('formatCount formatter', () => {
    it('formats counts in thousands (K)', () => {
      expect(formatCount(1500)).toBe('1.5K');
      expect(formatCount(10000)).toBe('10.0K');
    });

    it('formats counts in millions (M)', () => {
      expect(formatCount(2500000)).toBe('2.5M');
    });

    it('formats sub-thousand counts directly', () => {
      expect(formatCount(42)).toBe('42');
    });
  });

  describe('getStatusConfig', () => {
    const mockT = (key: string, params?: Record<string, string | number>) => {
      if (params?.count) return `${key}:${params.count}`;
      return key;
    };

    it('returns correct config for Safe status', () => {
      const config = getStatusConfig({ type: 'Safe', checked_at: '' }, mockT);
      expect(config.textColor).toContain('text-green-500');
      expect(config.shortLabel).toBe('breach.safe');
    });

    it('returns correct config for Breached status', () => {
      const config = getStatusConfig({ type: 'Breached', breach_count: 500, checked_at: '' }, mockT);
      expect(config.textColor).toContain('text-red-500');
      expect(config.shortLabel).toBe('breach.count_short:500');
    });
    it('returns reused warning config instead of safe when password is reused in vault', () => {
      const config = getStatusConfig(
        { type: 'Safe', checked_at: '' },
        mockT,
        { isReused: true, reusedServices: 'Main' }
      );
      expect(config.textColor).toContain('text-purple-400');
      expect(config.shortLabel).toBe('breach.reused_status');
      expect(config.label).toBe('breach.no_breaches_reused');
    });

    it('returns reused warning config even if breach status is Unknown', () => {
      const config = getStatusConfig(
        { type: 'Unknown' },
        mockT,
        { isReused: true, reusedServices: 'Steam' }
      );
      expect(config.textColor).toContain('text-purple-400');
      expect(config.shortLabel).toBe('breach.reused_status');
    });

    it('returns weak warning config instead of safe when password is weak', () => {
      const config = getStatusConfig(
        { type: 'Safe', checked_at: '' },
        mockT,
        { isWeak: true }
      );
      expect(config.textColor).toContain('text-amber-400');
      expect(config.shortLabel).toBe('security.stat_weak');
      expect(config.label).toBe('breach.no_breaches_weak');
    });
  });

  describe('getLocalizedIssueDescription', () => {
    const mockT = (key: string, params?: Record<string, string | number>) => {
      if (params?.count && params?.services) return `Reused across ${params.count} accounts: ${params.services}`;
      if (params?.count) return `Found in ${params.count} breaches`;
      if (params?.services) return `Reused with ${params.services}`;
      if (params?.days) return `Older than ${params.days} days`;
      return key;
    };

    it('formats breached issue description', () => {
      const issue: SecurityIssue = {
        entry_id: '1',
        entry_title: 'Test',
        issue_type: 'Breached',
        severity: 'Critical',
        description: 'Leaked in 12 breaches',
      };
      expect(getLocalizedIssueDescription(issue, mockT)).toBe('Found in 12 breaches');
    });

    it('formats reused password description with services list', () => {
      const issue: SecurityIssue = {
        entry_id: '1',
        entry_title: 'Test',
        issue_type: 'ReusedPassword',
        severity: 'Warning',
        description: 'Reused with: Google, GitHub',
      };
      expect(getLocalizedIssueDescription(issue, mockT)).toBe('Reused with Google, GitHub');
    });

    it('formats grouped reused password issue description with all account names', () => {
      const groupedIssue = {
        id: 'group-1',
        issue_type: 'ReusedPassword',
        severity: 'Warning',
        entry_id: '1',
        entry_title: 'Github, Main',
        description: 'Password is reused on: Main',
        is_group: true,
        group_entries: [
          { id: '1', title: 'Github' },
          { id: '2', title: 'Main' },
        ],
      };
      expect(getLocalizedIssueDescription(groupedIssue, mockT)).toBe(
        'Reused across 2 accounts: Github, Main'
      );
    });
  });
});
