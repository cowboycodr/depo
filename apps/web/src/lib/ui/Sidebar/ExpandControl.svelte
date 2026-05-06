<script lang="ts">
  import type { Snippet } from 'svelte';
  import { slide } from 'svelte/transition';
  import { getSidebarContext } from '@/ui/Sidebar/context';
  import ExpandButton from './ExpandButton.svelte';

  const {
    children
  }: {
    children?: Snippet;
  } = $props();

  const sidebar = getSidebarContext();
</script>

{#if !sidebar.open()}
  <div in:slide={{ duration: 150, axis: 'x' }} out:slide={{ duration: 100, axis: 'x' }}>
    {#if children}
      {@render children()}
    {:else}
      <ExpandButton />
    {/if}
  </div>
{/if}
