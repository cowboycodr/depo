<script lang="ts">
  import type { Snippet } from 'svelte';
  import { getCapsuleContext } from '@/ui/Capsule/context';
  import { cn } from '@/utils';

  let {
    side,
    compact = false,
    class: className,
    children
  }: {
    side: 'left' | 'right';
    compact?: boolean;
    class?: string;
    children: Snippet;
  } = $props();

  const capsule = getCapsuleContext();
  const surfaceClass = $derived(
    capsule.variant() === 'secondary' ? 'bg-surface' : 'bg-surface-muted'
  );
  const sideClass = $derived(
    side === 'left' ? 'z-0 -mr-1.5 rounded-l-ui' : 'z-0 -ml-1.5 rounded-r-ui'
  );
  const spacingClass = $derived(
    side === 'left' ? (compact ? 'py-0.5 pr-2 pl-0.5' : 'py-0 pr-4.5 pl-3') : 'py-0 pr-3 pl-4.5'
  );
</script>

<div
  class={cn(
    'relative inline-flex h-7 items-center gap-1.75 font-mono text-ui text-fg-muted',
    surfaceClass,
    sideClass,
    spacingClass,
    className
  )}
>
  {@render children()}
</div>
