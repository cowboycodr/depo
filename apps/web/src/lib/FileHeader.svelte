<script lang="ts">
  import Plus from '~icons/lucide/plus';
  import Columns2 from '~icons/lucide/columns-2';
  import Rows2 from '~icons/lucide/rows-2';
  import FileTab from '@/FileTab.svelte';
  import * as Capsule from '@/ui/Capsule';
  import * as Sidebar from '@/ui/Sidebar';
  import * as Toggle from '@/ui/Toggle';

  const formatBytes = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    return `${(bytes / 1024).toFixed(1)} KB`;
  };

  let {
    diffStyle = $bindable(),
    mode = 'file',
    tabs,
    activePath,
    onCloseTab,
    onReorderTabs,
    lines = 0,
    size = 0,
    additions = 0,
    removals = 0
  }: {
    diffStyle: 'split' | 'unified';
    mode?: 'file' | 'diff';
    tabs: Array<{ path: string; href: string }>;
    activePath: string | null;
    onCloseTab: (path: string) => void;
    onReorderTabs: (fromIndex: number, toIndex: number) => void;
    lines?: number;
    size?: number;
    additions?: number;
    removals?: number;
  } = $props();

  const formattedSize = $derived(formatBytes(size));

  let dragIndex = $state<number | null>(null);
  let dropIndex = $state<number | null>(null);
  let dropX = $state<number | null>(null);

  function handleDragStart(e: DragEvent, index: number) {
    if ((e.target as HTMLElement).closest('button') !== null) {
      e.preventDefault();
      return;
    }
    dragIndex = index;
    e.dataTransfer!.effectAllowed = 'move';
    e.dataTransfer!.setData('text/plain', String(index));
  }

  function handleDragEnd() {
    dragIndex = null;
    dropIndex = null;
    dropX = null;
  }

  function handleContainerDragOver(e: DragEvent) {
    e.preventDefault();
    e.dataTransfer!.dropEffect = 'move';
    const container = e.currentTarget as HTMLElement;
    const containerLeft = container.getBoundingClientRect().left;
    const tabEls = container.querySelectorAll<HTMLElement>('[data-tab-index]');
    let insert = tabs.length;
    let x = 0;
    for (const el of tabEls) {
      const rect = el.getBoundingClientRect();
      if (e.clientX < rect.left + rect.width / 2) {
        insert = Number(el.dataset.tabIndex);
        x = rect.left - containerLeft;
        break;
      }
      x = rect.right - containerLeft;
    }
    dropIndex = insert;
    dropX = x;
  }

  function handleContainerDrop(e: DragEvent) {
    e.preventDefault();
    if (dragIndex !== null && dropIndex !== null) {
      const toIndex = dragIndex < dropIndex ? dropIndex - 1 : dropIndex;
      if (toIndex !== dragIndex) {
        onReorderTabs(dragIndex, toIndex);
      }
    }
    dragIndex = null;
    dropIndex = null;
    dropX = null;
  }

  function handleContainerDragLeave(e: DragEvent) {
    if (!(e.currentTarget as HTMLElement).contains(e.relatedTarget as Node)) {
      dropIndex = null;
      dropX = null;
    }
  }
</script>

<div class="flex h-9.5 shrink-0 items-center justify-between gap-4 bg-surface-muted p-1.5">
  <Capsule.Root variant="secondary">
    {#if tabs.length > 0}
      <Sidebar.ExpandControl>
        <Capsule.Extension side="left" compact class="pr-2">
          <Sidebar.ExpandButton />
        </Capsule.Extension>
      </Sidebar.ExpandControl>
    {:else}
      <Sidebar.ExpandControl />
    {/if}

    {#if tabs.length > 0}
      <Capsule.Core>
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="relative flex"
          ondragover={handleContainerDragOver}
          ondrop={handleContainerDrop}
          ondragleave={handleContainerDragLeave}
        >
          {#each tabs as tab, i (tab.path)}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              data-tab-index={i}
              draggable="true"
              class="cursor-grab active:cursor-grabbing"
              ondragstart={(e) => handleDragStart(e, i)}
              ondragend={handleDragEnd}
            >
              <FileTab
                path={tab.path}
                href={tab.href}
                active={tab.path === activePath}
                dragging={dragIndex === i}
                onclose={() => onCloseTab(tab.path)}
              />
            </div>
          {/each}
          {#if dragIndex !== null && dropX !== null}
            <div
              class="pointer-events-none absolute inset-y-0.5 w-0.5 -translate-x-1/2 rounded-full bg-accent"
              style="left: {dropX}px"
            ></div>
          {/if}
        </div>
      </Capsule.Core>

      <Capsule.Extension side="right" class="py-0.5 pl-2 pr-0.5">
        <button
          disabled
          class="flex h-6 w-6 cursor-not-allowed items-center justify-center rounded-md text-fg-subtle opacity-40"
          aria-label="Open file"
        >
          <Plus width={12} height={12} stroke-width={2} />
        </button>
      </Capsule.Extension>
    {/if}
  </Capsule.Root>

  <div class="flex shrink-0 items-center gap-2 font-mono whitespace-nowrap">
    {#if activePath}
      <div class="mr-0.5 flex items-center gap-1.75 text-ui text-fg-secondary">
        <span>{lines} lines</span>
        <span>{formattedSize}</span>
      </div>
    {/if}

    {#if mode === 'diff'}
      <div class="flex items-center gap-1.75 text-ui-sm text-fg-subtle">
        {#if additions > 0}
          <span class="text-diff-add-strong">+{additions}</span>
        {/if}
        {#if removals > 0}
          <span class="text-danger">&#8722;{removals}</span>
        {/if}
      </div>

      <Toggle.Root bind:value={diffStyle}>
        <Toggle.Button value="split" label="Horizontal diff view" title="Horizontal">
          <Columns2 width={13} height={13} stroke-width={2} />
        </Toggle.Button>

        <Toggle.Button value="unified" label="Stacked diff view" title="Stacked">
          <Rows2 width={13} height={13} stroke-width={2} />
        </Toggle.Button>
      </Toggle.Root>
    {/if}
  </div>
</div>
