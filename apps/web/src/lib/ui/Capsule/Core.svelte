<script lang="ts">
  import type { Snippet } from 'svelte';
  import { cn } from '@/utils';

  const { class: className, children }: { class?: string; children: Snippet } = $props();
</script>

<div
  class={cn(
    'relative z-[1] flex h-7 min-w-0 items-center gap-0.5 rounded-ui bg-surface p-0.5',
    className
  )}
>
  {@render children()}
</div>
