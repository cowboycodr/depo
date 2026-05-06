<script lang="ts">
  import type { Snippet } from 'svelte';
  import { setSidebarContext } from '@/ui/Sidebar/context';

  let {
    open = $bindable(true),
    children
  }: {
    open?: boolean;
    children: Snippet;
  } = $props();

  setSidebarContext({
    open: () => open,
    setOpen: (nextOpen) => (open = nextOpen)
  });
</script>

{@render children()}
