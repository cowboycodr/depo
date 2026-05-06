<script lang="ts">
  import type { Snippet } from 'svelte';
  import { setCapsuleContext, type CapsuleVariant } from '@/ui/Capsule/context';
  import { cn } from '@/utils';

  let {
    variant = 'primary',
    class: className,
    children
  }: {
    variant?: CapsuleVariant;
    class?: string;
    children: Snippet;
  } = $props();

  setCapsuleContext({ variant: () => variant });
</script>

<div
  class={cn(
    'relative inline-flex h-7 min-w-0 items-center self-center whitespace-nowrap',
    className
  )}
>
  {@render children()}
</div>
