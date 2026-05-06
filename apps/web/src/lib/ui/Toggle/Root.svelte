<script lang="ts">
  import type { Snippet } from 'svelte';
  import { setToggleContext } from '@/ui/Toggle/context';

  let {
    value = $bindable(),
    children
  }: {
    value: string;
    children: Snippet;
  } = $props();

  setToggleContext({
    value: () => value,
    setValue: (nextValue) => (value = nextValue)
  });
</script>

<div class="flex h-7 items-center gap-px rounded-ui bg-surface p-0.5">
  {@render children()}
</div>
