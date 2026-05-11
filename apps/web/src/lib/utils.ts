import { type ClassValue, clsx } from 'clsx';
import { extendTailwindMerge } from 'tailwind-merge';

const twMerge = extendTailwindMerge({
  extend: {
    classGroups: {
      'font-size': [
        'text-2xs',
        'text-badge',
        'text-ui-xs',
        'text-ui-sm',
        'text-ui',
        'text-ui-md',
        'text-code'
      ]
    }
  }
});

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export const MS_PER_DAY = 86_400_000;

export function countLines(value: string): number {
  if (value.length === 0) return 0;
  return value.endsWith('\n') ? value.slice(0, -1).split('\n').length : value.split('\n').length;
}

export function reorderTabs(arr: string[], fromIndex: number, toIndex: number): string[] {
  const next = [...arr];
  const [item] = next.splice(fromIndex, 1);
  if (item !== undefined) {
    next.splice(toIndex, 0, item);
  }
  return next;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  return `${(bytes / 1024).toFixed(1)} KB`;
}

export function commitDate(iso: string): string {
  return new Date(iso).toLocaleString('en', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
    hour: 'numeric',
    minute: '2-digit'
  });
}
