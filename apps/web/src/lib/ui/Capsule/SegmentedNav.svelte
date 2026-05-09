<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { SvelteMap } from 'svelte/reactivity';
  import { resolve } from '$app/paths';
  import type { AppHref } from '@/navigation';
  import { cn } from '@/utils';

  export type SegmentedNavItem = {
    id: string;
    label: string;
    href?: AppHref;
    disabled?: boolean;
    count?: number;
  };

  const {
    items,
    activeId,
    onSelect,
    class: className
  }: {
    items: SegmentedNavItem[];
    activeId: string;
    onSelect?: (id: string) => void;
    class?: string;
  } = $props();

  let container: HTMLDivElement;
  let resizeObserver: ResizeObserver | null = null;
  const itemElements = new SvelteMap<string, HTMLElement>();
  let indicator = $state({ left: 0, width: 0, visible: false });

  const indicatorStyle = $derived(
    `width: ${indicator.width}px; transform: translateX(${indicator.left}px); opacity: ${
      indicator.visible ? 1 : 0
    };`
  );

  function updateIndicator() {
    if (!container) return;

    const activeElement = itemElements.get(activeId);
    if (activeElement === undefined) {
      indicator = { ...indicator, visible: false };
      return;
    }

    const containerRect = container.getBoundingClientRect();
    const activeRect = activeElement.getBoundingClientRect();
    indicator = {
      left: activeRect.left - containerRect.left,
      width: activeRect.width,
      visible: true
    };
  }

  function trackItem(node: HTMLElement, id: string) {
    itemElements.set(id, node);
    resizeObserver?.observe(node);
    void tick().then(updateIndicator);

    return {
      update(nextId: string) {
        itemElements.delete(id);
        id = nextId;
        itemElements.set(id, node);
        void tick().then(updateIndicator);
      },
      destroy() {
        resizeObserver?.unobserve(node);
        itemElements.delete(id);
        void tick().then(updateIndicator);
      }
    };
  }

  function selectItem(id: string, disabled = false) {
    if (!disabled) onSelect?.(id);
  }

  function scheduleIndicatorUpdate(_activeId: string, _items: SegmentedNavItem[]) {
    void tick().then(updateIndicator);
  }

  const itemClass = (active: boolean, disabled = false) =>
    cn(
      'relative z-10 flex h-6 select-none items-center rounded-md px-2.25 text-ui-md no-underline transition-colors duration-150',
      active ? 'text-fg-bright' : 'text-fg-muted',
      !active && !disabled ? 'hover:bg-surface-hover hover:text-fg-secondary' : undefined,
      disabled ? 'cursor-not-allowed opacity-40' : undefined
    );

  $effect(() => {
    scheduleIndicatorUpdate(activeId, items);
  });

  onMount(() => {
    resizeObserver = new ResizeObserver(updateIndicator);
    resizeObserver.observe(container);
    for (const element of itemElements.values()) {
      resizeObserver.observe(element);
    }
    updateIndicator();

    return () => {
      resizeObserver?.disconnect();
      resizeObserver = null;
    };
  });
</script>

<div
  bind:this={container}
  class={cn(
    'relative z-[1] flex h-7 min-w-0 items-center gap-0.5 rounded-ui bg-surface p-0.5',
    className
  )}
>
  <span
    aria-hidden="true"
    class="pointer-events-none absolute inset-y-0.5 left-0 rounded-md bg-surface-nav-active shadow-ring will-change-transform"
    style={`${indicatorStyle} transition: transform 180ms cubic-bezier(0.2, 0.8, 0.2, 1), width 180ms cubic-bezier(0.2, 0.8, 0.2, 1), opacity 120ms ease-out;`}
  ></span>

  {#each items as item (item.id)}
    {@const active = item.id === activeId}
    {#if item.href !== undefined && !item.disabled}
      <a
        use:trackItem={item.id}
        href={resolve(item.href)}
        class={itemClass(active)}
        aria-current={active ? 'page' : undefined}
        onclick={() => selectItem(item.id)}
      >
        {item.label}
        {#if item.count !== undefined}
          <span
            class="ml-1.5 rounded-badge bg-surface-chip px-1.25 font-mono text-badge leading-badge"
          >
            {item.count}
          </span>
        {/if}
      </a>
    {:else}
      <button
        use:trackItem={item.id}
        type="button"
        class={itemClass(active, item.disabled)}
        disabled={item.disabled}
        aria-current={active ? 'page' : undefined}
        onclick={() => selectItem(item.id, item.disabled)}
      >
        {item.label}
        {#if item.count !== undefined}
          <span
            class="ml-1.5 rounded-badge bg-surface-chip px-1.25 font-mono text-badge leading-badge"
          >
            {item.count}
          </span>
        {/if}
      </button>
    {/if}
  {/each}
</div>
