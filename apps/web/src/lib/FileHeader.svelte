<script lang="ts">
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
    fileName = 'README.md',
    lines = 0,
    size = 0,
    additions = 0,
    removals = 0
  }: {
    diffStyle: 'split' | 'unified';
    mode?: 'file' | 'diff';
    fileName?: string;
    lines?: number;
    size?: number;
    additions?: number;
    removals?: number;
  } = $props();

  const formattedSize = $derived(formatBytes(size));
</script>

<div class="flex h-9.5 shrink-0 items-center justify-between gap-4 bg-surface-muted p-1.5">
  <Capsule.Root variant="secondary">
    <Sidebar.ExpandControl>
      <Capsule.Extension side="left" compact class="pr-2">
        <Sidebar.ExpandButton />
      </Capsule.Extension>
    </Sidebar.ExpandControl>

    <Capsule.Core>
      <FileTab {fileName} />
    </Capsule.Core>
  </Capsule.Root>

  <div class="flex shrink-0 items-center gap-2 font-mono whitespace-nowrap">
    <div class="mr-0.5 flex items-center gap-1.75 text-ui text-fg-secondary">
      <span>{lines} lines</span>
      <span>{formattedSize}</span>
    </div>

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
