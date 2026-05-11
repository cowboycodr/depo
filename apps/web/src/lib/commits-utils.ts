import type { CommitSummary } from '@depo/api-client';
import { MS_PER_DAY } from './utils';

export type CommitGroup = { label: string; commits: CommitSummary[] };

export function groupLabel(isoDate: string): string {
  const date = new Date(isoDate);
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const d = new Date(date.getFullYear(), date.getMonth(), date.getDate());
  const diff = Math.round((today.getTime() - d.getTime()) / MS_PER_DAY);
  if (diff === 0) return 'Today';
  if (diff === 1) return 'Yesterday';
  if (diff < 7) return d.toLocaleDateString('en', { weekday: 'long' });
  if (d.getFullYear() === now.getFullYear())
    return d.toLocaleDateString('en', { month: 'long', day: 'numeric' });
  return d.toLocaleDateString('en', { month: 'long', day: 'numeric', year: 'numeric' });
}

export function groupByDate(commits: CommitSummary[]): CommitGroup[] {
  const groups: CommitGroup[] = [];
  let currentLabel = '';
  for (const commit of commits) {
    const label = groupLabel(commit.committedAt);
    if (label !== currentLabel) {
      currentLabel = label;
      groups.push({ label, commits: [commit] });
    } else {
      const last = groups[groups.length - 1];
      if (last) last.commits.push(commit);
    }
  }
  return groups;
}

export function formatFullDate(isoDate: string): string {
  return new Date(isoDate).toLocaleString(undefined, {
    year: 'numeric',
    month: 'long',
    day: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
    second: '2-digit'
  });
}

export function timeAgo(isoDate: string): string {
  const ms = Date.now() - new Date(isoDate).getTime();
  const s = Math.floor(ms / 1000);
  const m = Math.floor(s / 60);
  const h = Math.floor(m / 60);
  const d = Math.floor(h / 24);
  if (s < 60) return 'just now';
  if (m < 60) return `${m}m ago`;
  if (h < 24) return `${h}h ago`;
  if (d < 30) return `${d}d ago`;
  if (d < 365) return `${Math.floor(d / 30)}mo ago`;
  return `${Math.floor(d / 365)}y ago`;
}

type Palette = { bg: string; text: string };

const AVATAR_PALETTES: Palette[] = [
  { bg: '#1a2e20', text: '#4ade80' },
  { bg: '#1a2035', text: '#60a5fa' },
  { bg: '#26183a', text: '#c084fc' },
  { bg: '#301a26', text: '#f472b6' },
  { bg: '#2e2018', text: '#fb923c' },
  { bg: '#272e18', text: '#a3e635' }
];

export function avatarPalette(name: string): Palette {
  let h = 0;
  for (let i = 0; i < name.length; i++) h = (Math.imul(31, h) + name.charCodeAt(i)) | 0;
  return AVATAR_PALETTES[Math.abs(h) % AVATAR_PALETTES.length] ?? AVATAR_PALETTES[0]!;
}

export function initials(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  const first = parts[0] ?? '';
  const last = parts.length > 1 ? (parts[parts.length - 1] ?? '') : '';
  if (parts.length <= 1) return (first[0] ?? '?').toUpperCase();
  return ((first[0] ?? '') + (last[0] ?? '')).toUpperCase();
}
