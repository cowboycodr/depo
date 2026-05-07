<script lang="ts">
  import type { Snippet } from 'svelte';

  import { cn } from '@/utils';

  const {
    active = false,
    disabled = false,
    count = undefined,
    href = undefined,
    class: className,
    children
  }: {
    active?: boolean;
    disabled?: boolean;
    count?: number;
    href?: string;
    class?: string;
    children: Snippet;
  } = $props();

  const cls = $derived(
    cn(
      'relative flex h-6 select-none items-center rounded-md px-2.25 text-ui-md text-fg-muted no-underline hover:bg-surface-hover hover:text-fg-secondary data-[active=true]:bg-surface-nav-active data-[active=true]:text-fg-bright data-[active=true]:shadow-ring data-[disabled=true]:pointer-events-none data-[disabled=true]:opacity-40',
      className
    )
  );
</script>

{#if href !== undefined}
  <a {href} class={cls} data-active={active} data-disabled={disabled}>
    {@render children()}
    {#if count !== undefined}
      <span class="ml-1.5 rounded-badge bg-surface-chip px-1.25 font-mono text-badge leading-badge">
        {count}
      </span>
    {/if}
  </a>
{:else}
  <div class={cls} data-active={active} data-disabled={disabled}>
    {@render children()}
    {#if count !== undefined}
      <span class="ml-1.5 rounded-badge bg-surface-chip px-1.25 font-mono text-badge leading-badge">
        {count}
      </span>
    {/if}
  </div>
{/if}
