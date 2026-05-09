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
    class="w-70 flex-none overflow-hidden bg-canvas"
    transition:slide={{ axis: 'x', duration: 200 }}
  >
    <div class="flex h-full flex-col">
      <div class="flex h-9.5 shrink-0 items-center border-b border-line px-1.5">
        {#if controls}
          {@render controls()}
        {:else}
          <div class="ml-auto">
            <CollapseButton />
          </div>
        {/if}
      </div>

      <div class="min-h-0 flex-1">
        {@render children()}
      </div>
    </div>
  </aside>
{/if}
