<script lang="ts">
  import type { Snippet } from 'svelte';
  import { getPopoverContext } from './context';

  let {
    children,
    align = 'end',
    class: className
  }: {
    children: Snippet;
    align?: 'start' | 'end';
    class?: string;
  } = $props();

  const popover = getPopoverContext();
</script>

{#if popover.visible()}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    role="tooltip"
    class={[
      'absolute top-full z-50 mt-1.5',
      align === 'end' ? 'right-0' : 'left-0',
      className
    ]
      .filter(Boolean)
      .join(' ')}
    onmouseenter={popover.show}
    onmouseleave={popover.scheduleHide}
  >
    {@render children()}
  </div>
{/if}
