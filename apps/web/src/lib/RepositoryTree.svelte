<script lang="ts">
  import ChevronDown from '~icons/lucide/chevron-down';
  import ChevronRight from '~icons/lucide/chevron-right';
  import GitBranch from '~icons/lucide/git-branch';
  import { SvelteSet } from 'svelte/reactivity';
  import type { TreeEntry } from '@depo/api-client';
  import LanguageIcon from '@/LanguageIcon.svelte';
  import {
    ancestorsForPath,
    createTree,
    getVisibleRows,
    hasChangedDescendant,
    pathsFromEntries,
    type VisibleNode
  } from '@/repository-tree';

  const defaultHrefForPath = (path: string) => {
    const query = new URLSearchParams({ path });
    return `?${query.toString()}`;
  };

  const {
    nodes = [],
    selectedPath = '',
    changedPaths: changedPathsProp,
    hrefForPath = defaultHrefForPath
  }: {
    nodes?: TreeEntry[];
    selectedPath?: string;
    changedPaths?: string[];
    hrefForPath?: (path: string) => string;
  } = $props();

  const paths = $derived(pathsFromEntries(nodes));
  const tree = $derived(createTree(paths));
  const changedPaths = $derived(
    changedPathsProp !== undefined
      ? new Set(changedPathsProp)
      : new Set(selectedPath ? [selectedPath] : [])
  );
  const openFolders = new SvelteSet<string>();

  const isOpen = (path: string) => openFolders.has(path);

  const toggleFolder = (path: string) => {
    if (openFolders.has(path)) {
      openFolders.delete(path);
    } else {
      openFolders.add(path);
    }
  };

  const visibleRows = $derived(getVisibleRows(tree, isOpen, changedPaths));

  $effect(() => {
    for (const path of ancestorsForPath(selectedPath)) {
      openFolders.add(path);
    }
  });

  const rowClass = (selected: boolean) =>
    [
      'group grid h-[23px] w-full grid-cols-[1fr_auto] items-center gap-2 rounded-avatar pr-2.5 text-left text-ui-md leading-none outline-none transition-colors duration-[120ms] focus-visible:shadow-ring',
      selected
        ? 'bg-surface-hover font-medium text-fg'
        : 'font-[430] text-fg-secondary hover:bg-overlay-hover hover:text-fg'
    ].join(' ');

  const twistClass = (selected: boolean) =>
    [
      'inline-flex h-3.5 w-3.5 shrink-0 items-center justify-center',
      selected ? 'text-fg-caret' : 'text-fg-caret group-hover:text-fg'
    ].join(' ');
</script>

{#snippet rowContent(
  row: VisibleNode,
  selected: boolean,
  folder: boolean,
  open: boolean,
  changed: boolean
)}
  <span class="flex min-w-0 items-center gap-1.5">
    <span class={twistClass(selected)}>
      {#if folder}
        {#if open}
          <ChevronDown width={13} height={13} stroke-width={2.1} />
        {:else}
          <ChevronRight width={13} height={13} stroke-width={2.1} />
        {/if}
      {/if}
    </span>

    {#if !folder && row.node.name.startsWith('.git')}
      <span class="inline-flex h-3.75 w-3.75 shrink-0 items-center justify-center text-danger">
        <GitBranch width={14} height={14} stroke-width={2} />
      </span>
    {:else if !folder}
      <LanguageIcon name={row.node.name} />
    {/if}

    <span class="min-w-0 truncate">{row.displayName}</span>
  </span>

  <span class="flex shrink-0 items-center gap-1.5 font-mono text-ui-xs text-fg-tertiary">
    {#if changed && !selected}
      <span class="block h-1.5 w-1.5 rounded-full bg-diff-add" aria-label="Contains changes"></span>
    {/if}
  </span>
{/snippet}

<div
  class="h-full min-h-0 w-full overflow-y-auto py-3 px-2 text-fg-secondary [scrollbar-gutter:stable] [&::-webkit-scrollbar-thumb]:rounded-full [&::-webkit-scrollbar-thumb]:bg-transparent [&::-webkit-scrollbar]:w-2 hover:[&::-webkit-scrollbar-thumb]:bg-surface-hover"
>
  <div class="space-y-0.5">
    {#each visibleRows as row (row.node.path)}
      {@const selected = row.node.path === selectedPath}
      {@const folder = row.node.type === 'folder'}
      {@const open = folder && isOpen(row.node.path)}
      {@const changed = hasChangedDescendant(row.node, changedPaths)}

      {#if folder}
        <button
          type="button"
          class={rowClass(selected)}
          style={`padding-left: ${10 + row.depth * 18}px`}
          aria-current={selected ? 'page' : undefined}
          aria-expanded={open}
          onclick={() => toggleFolder(row.node.path)}
        >
          {@render rowContent(row, selected, folder, open, changed)}
        </button>
      {:else}
        <a
          class={rowClass(selected)}
          style={`padding-left: ${10 + row.depth * 18}px`}
          aria-current={selected ? 'page' : undefined}
          href={hrefForPath(row.node.path)}
        >
          {@render rowContent(row, selected, folder, open, changed)}
        </a>
      {/if}
    {/each}
  </div>
</div>
