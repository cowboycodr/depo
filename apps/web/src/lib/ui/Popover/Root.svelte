<script lang="ts">
  import type { Snippet } from 'svelte';
  import { setPopoverContext } from './context';

  let {
    children,
    class: className
  }: {
    children: Snippet;
    class?: string;
  } = $props();

  let visible = $state(false);
  let hideTimer: ReturnType<typeof setTimeout> | null = null;

  function show() {
    if (hideTimer !== null) {
      clearTimeout(hideTimer);
      hideTimer = null;
    }
    visible = true;
  }

  function scheduleHide() {
    hideTimer = setTimeout(() => {
      visible = false;
      hideTimer = null;
    }, 150);
  }

  setPopoverContext({ visible: () => visible, show, scheduleHide });
</script>

<div
  role="presentation"
  class={['relative', className].filter(Boolean).join(' ')}
  onmouseenter={show}
  onmouseleave={scheduleHide}
>
  {@render children()}
</div>
