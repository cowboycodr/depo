<script lang="ts">
  import { resolve } from '$app/paths';
  import X from '~icons/lucide/x';
  import LanguageIcon from '@/LanguageIcon.svelte';
  import type { AppHref } from '@/navigation';

  const {
    path,
    href,
    active,
    dragging = false,
    onclose
  }: {
    path: string;
    href: AppHref;
    active: boolean;
    dragging?: boolean;
    onclose: () => void;
  } = $props();

  const baseName = $derived(path.split('/').at(-1) ?? path);
  const lastDot = $derived(baseName.lastIndexOf('.'));
  const base = $derived(lastDot > 0 ? baseName.slice(0, lastDot) : baseName);
  const ext = $derived(lastDot > 0 ? baseName.slice(lastDot) : '');

  function handleClose(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    onclose();
  }
</script>

<a
  href={resolve(href)}
  draggable="false"
  class="group relative flex h-6 max-w-48 select-none items-center gap-1 rounded-md pl-2 pr-2 text-ui-md text-fg-muted no-underline transition-[background-color,color,box-shadow,opacity] duration-150 hover:bg-overlay-hover hover:text-fg-secondary data-[active=true]:bg-surface-nav-active data-[active=true]:text-fg-bright data-[active=true]:shadow-control"
  class:opacity-40={dragging}
  data-active={active}
>
  <span class="relative flex h-3.75 w-3.75 shrink-0 items-center justify-center">
    <span
      class="pointer-events-none flex items-center justify-center transition-opacity duration-100 group-hover:opacity-0"
    >
      <LanguageIcon name={baseName} />
    </span>
    <button
      class="absolute inset-0 flex items-center justify-center opacity-0 transition-opacity duration-100 group-hover:opacity-100 hover:text-fg-bright"
      onclick={handleClose}
      onmousedown={(e) => e.stopPropagation()}
      tabindex="-1"
      aria-label="Close tab"
    >
      <X width={10} height={10} stroke-width={2.5} />
    </button>
  </span>
  <span class="truncate font-medium">
    {base}<span class="font-normal text-fg-secondary">{ext}</span>
  </span>
</a>
