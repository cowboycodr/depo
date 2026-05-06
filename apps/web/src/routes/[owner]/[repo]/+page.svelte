<script lang="ts">
  import type { PageData } from './$types';
  import FileHeader from '@/FileHeader.svelte';
  import FileViewer from '@/FileViewer.svelte';
  import NavBar from '@/NavBar.svelte';
  import RepositoryTree from '@/RepositoryTree.svelte';
  import * as Sidebar from '@/ui/Sidebar';

  const { data }: { data: PageData } = $props();

  let diffStyle: 'split' | 'unified' = $state('unified');
  let sidebarOpen = $state(true);

  const activeFile = $derived(data.view?.activeFile ?? null);
  const activePath = $derived(activeFile?.path ?? data.path ?? '');
  const content = $derived(activeFile?.content ?? '');
  const lineCount = $derived(countLines(content));
  const hrefForPath = (path: string) => {
    const query = new URLSearchParams();
    if (data.ref) query.set('ref', data.ref);
    query.set('path', path);
    return `?${query.toString()}`;
  };

  function countLines(value: string) {
    if (value.length === 0) return 0;
    return value.endsWith('\n') ? value.slice(0, -1).split('\n').length : value.split('\n').length;
  }
</script>

<svelte:head>
  <title>{data.owner}/{data.repo}</title>
  <meta name="description" content={`Depo repository view for ${data.owner}/${data.repo}.`} />
</svelte:head>

<div class="grid h-full grid-rows-[42px_minmax(0,1fr)]">
  <NavBar
    owner={data.owner}
    repo={data.repo}
    refName={data.view?.ref.name ?? 'main'}
    commitSha={data.view?.ref.commitSha ?? null}
  />

  <Sidebar.Root bind:open={sidebarOpen}>
    <div class="flex h-[calc(100vh-42px)] overflow-hidden bg-canvas">
      <Sidebar.Panel>
        <RepositoryTree
          nodes={data.view?.tree.nodes ?? []}
          selectedPath={activePath}
          {hrefForPath}
        />
      </Sidebar.Panel>

      <main class="relative min-w-0 flex-1 overflow-hidden">
        <div
          class={[
            'relative grid h-full grid-rows-[auto_minmax(0,1fr)] overflow-hidden rounded-tl-ui bg-surface-muted [transition:border-top-right-radius_200ms_ease-in-out]',
            !sidebarOpen ? 'rounded-tr-ui' : ''
          ].join(' ')}
        >
          <FileHeader
            bind:diffStyle
            fileName={activeFile?.path ?? (activePath || 'Repository')}
            lines={lineCount}
            size={activeFile?.size ?? 0}
          />
          <div class="flex min-h-0 flex-1 flex-col overflow-hidden">
            {#if data.error}
              <div class="flex h-full items-center justify-center bg-surface-muted p-8 text-ui text-fg-secondary">
                {data.error.message}
              </div>
            {:else if activeFile?.kind === 'text' && activeFile.content !== null}
              {#key `${activeFile.commitSha}:${activeFile.path}`}
                <FileViewer content={activeFile.content} />
              {/key}
            {:else if activeFile}
              <div class="flex h-full items-center justify-center bg-surface-muted p-8 text-ui text-fg-secondary">
                {activeFile.path} cannot be previewed inline.
              </div>
            {:else}
              <div class="flex h-full items-center justify-center bg-surface-muted p-8 text-ui text-fg-secondary">
                Select a file from the tree.
              </div>
            {/if}
          </div>
        </div>
      </main>
    </div>
  </Sidebar.Root>
</div>
