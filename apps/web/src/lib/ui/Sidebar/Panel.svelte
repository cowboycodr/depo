<script lang="ts">
  import type { Snippet } from 'svelte';
  import { slide } from 'svelte/transition';
  import { getSidebarContext } from '@/ui/Sidebar/context';
  import CollapseButton from './CollapseButton.svelte';

  const {
    controls,
    children
  }: {
    controls?: Snippet;
    children: Snippet;
  } = $props();

  const sidebar = getSidebarContext();
</script>

{#if sidebar.open()}
  <aside
    class="flex-none overflow-hidden min-w-70 bg-canvas"
    transition:slide={{ axis: 'x', duration: 200 }}
  >
    <div class="flex h-full flex-col">
      <div class="flex h-9.5 shrink-0 items-center justify-end px-1">
        {#if controls}
          {@render controls()}
        {:else}
          <CollapseButton />
        {/if}
      </div>

      <div class="min-h-0 flex-1">
        {@render children()}
      </div>
    </div>
  </aside>
{/if}
