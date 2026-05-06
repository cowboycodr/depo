<script lang="ts">
  import type { Snippet } from 'svelte';
  import { cn } from '@/utils';

  type ButtonVariant = 'nav-action' | 'sidebar-collapse' | 'sidebar-expand' | 'toggle';

  const {
    variant = 'nav-action',
    label,
    title = label,
    active = undefined,
    type = 'button',
    onclick,
    class: className,
    children
  }: {
    variant?: ButtonVariant;
    label?: string;
    title?: string;
    active?: boolean;
    type?: 'button' | 'submit' | 'reset';
    onclick?: () => void;
    class?: string;
    children: Snippet;
  } = $props();

  const variantClass = $derived(
    {
      'nav-action':
        'inline-flex h-6 items-center gap-1.5 rounded-ui border-0 bg-surface px-2.25 font-sans text-ui-md leading-none whitespace-nowrap text-fg hover:bg-surface-hover',
      'sidebar-collapse':
        'inline-flex h-6 w-6 items-center justify-center rounded text-fg-tertiary transition-colors hover:bg-overlay-hover hover:text-fg',
      'sidebar-expand':
        'flex h-6 items-center justify-center rounded-md px-1.5 text-fg-muted transition-colors hover:bg-surface-hover hover:text-fg-secondary',
      toggle:
        'inline-flex h-6 w-6 select-none items-center justify-center rounded-md border-0 bg-transparent p-0 text-fg-muted hover:bg-surface-hover hover:text-fg-secondary data-[active=true]:bg-surface-nav-active data-[active=true]:text-fg-bright data-[active=true]:shadow-control'
    }[variant]
  );
</script>

<button
  {type}
  class={cn(variantClass, className)}
  data-active={active}
  aria-label={label}
  aria-pressed={active}
  {title}
  {onclick}
>
  {@render children()}
</button>
