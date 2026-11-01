import type { SecurityIssue } from '@/lib/backend';


export interface SecurityDashboardProps {
  onNavigateToEntry?: (entryId: string) => void;
  onOpenChangePassword?: () => void;
}


export interface DisplaySecurityIssue {
  id: string;
  issue_type: string;
  severity: string;
  entry_id: string;
  entry_title: string;
  description: string;
  is_group?: boolean;
  group_entries?: { id: string; title: string }[];
}


export function getLocalizedIssueDescription(
  issue: SecurityIssue | DisplaySecurityIssue,
  t: (key: string, params?: Record<string, string | number>) => string
): string {
  switch (issue.issue_type) {
    case 'Breached': {
      const match = issue.description.match(/\d+/);
      const count = match ? match[0] : '1';
      return t('security.desc_breached', { count });
    }
    case 'WeakPassword': {
      return t('security.desc_weak');
    }
    case 'ReusedPassword': {
      if ('is_group' in issue && issue.is_group && issue.group_entries) {
        const services = issue.group_entries.map((e: { title: string }) => e.title).join(', ');
        return t('security.reused_group_desc', { count: issue.group_entries.length, services });
      }
      const parts = issue.description.split(': ');
      const services = parts.length > 1 ? parts[1] : '';
      return services ? t('security.desc_reused_with', { services }) : t('security.desc_reused');
    }
    case 'OldPassword': {
      const match = issue.description.match(/\d+/);
      const days = match ? match[0] : '90';
      return t('security.desc_old_days', { days });
    }
    case 'Missing2FA': {
      return t('security.desc_missing_2fa');
    }
    default:
      return issue.description;
  }
}