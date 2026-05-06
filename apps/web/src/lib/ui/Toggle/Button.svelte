<script lang="ts">
  import type { Snippet } from 'svelte';
  import * as Button from '@/ui/Button';
  import { getToggleContext } from '@/ui/Toggle/context';

  const {
    value,
    label,
    title = label,
    children
  }: {
    value: string;
    label: string;
    title?: string;
    children: Snippet;
  } = $props();

  const toggle = getToggleContext();
  const active = $derived(toggle.value() === value);
</script>

<Button.Root variant="toggle" {label} {title} {active} onclick={() => toggle.setValue(value)}>
  {@render children()}
</Button.Root>
