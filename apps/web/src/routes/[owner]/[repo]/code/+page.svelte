<script lang="ts">
  import { goto } from '$app/navigation';
  import { untrack } from 'svelte';
  import type { PageData } from './$types';
  import FileHeader from '@/FileHeader.svelte';
  import FileViewer from '@/FileViewer.svelte';
  import NavBar from '@/NavBar.svelte';
  import RepositoryTree from '@/RepositoryTree.svelte';
  import * as Sidebar from '@/ui/Sidebar';
  import { countLines, reorderTabs } from '@/utils';

  const { data }: { data: PageData } = $props();

  let diffStyle: 'split' | 'unified' = $state('unified');
  let sidebarOpen = $state(true);
  let tabs = $state<string[]>([]);

  const activeFile = $derived(data.view?.activeFile ?? null);
  const activePath = $derived(activeFile?.path ?? data.path ?? null);
  const content = $derived(activeFile?.content ?? '');
  const lineCount = $derived(countLines(content));
  const commitBadge = $derived(
    activeFile?.lastCommit
      ? {
          sha: activeFile.lastCommit.sha,
          title: activeFile.lastCommit.title,
          href: `/${data.owner}/${data.repo}/commits/${activeFile.lastCommit.sha}`,
          author: activeFile.lastCommit.author,
          committedAt: activeFile.lastCommit.committedAt,
          additions: activeFile.lastCommit.additions,
          removals: activeFile.lastCommit.removals,
          description: activeFile.lastCommit.description
        }
      : undefined
  );

  const hrefForPath = (path: string) => {
    const query = new URLSearchParams();
    if (data.ref) query.set('ref', data.ref);
    query.set('path', path);
    return `?${query.toString()}`;
  };

  $effect(() => {
    const path = activeFile?.path;
    if (path !== undefined) {
      untrack(() => {
        if (!tabs.includes(path)) {
          tabs = [...tabs, path];
        }
      });
    }
  });

  function closeTab(path: string) {
    const idx = tabs.indexOf(path);
    const next = tabs.filter((t) => t !== path);
    tabs = next;
    if (activePath === path) {
      const destination = next[idx] ?? next[idx - 1];
      if (destination) {
        goto(hrefForPath(destination));
      } else {
        const query = new URLSearchParams();
        if (data.ref) query.set('ref', data.ref);
        query.set('nofile', '1');
        goto(`/${data.owner}/${data.repo}/code?${query.toString()}`);
      }
    }
  }

  function onReorderTabs(fromIndex: number, toIndex: number) {
    tabs = reorderTabs(tabs, fromIndex, toIndex);
  }
</script>

<svelte:head>
  <title>code:{data.owner}/{data.repo}</title>
  <meta name="description" content={`Depo repository view for ${data.owner}/${data.repo}.`} />
</svelte:head>

<div class="grid h-full grid-rows-[42px_minmax(0,1fr)]">
  <NavBar
    owner={data.owner}
    repo={data.repo}
    refName={data.view?.ref.name ?? 'main'}
    commitSha={data.view?.ref.commitSha ?? null}
    page="code"
    commitCount={data.view?.recentCommits.length}
  />

  <Sidebar.Root bind:open={sidebarOpen}>
    <div class="flex h-[calc(100vh-42px)] overflow-hidden bg-canvas">
      <Sidebar.Panel>
        <RepositoryTree
          nodes={data.view?.tree.nodes ?? []}
          selectedPath={activePath ?? undefined}
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
            tabs={tabs.map((p) => ({ path: p, href: hrefForPath(p) }))}
            {activePath}
            onCloseTab={closeTab}
            {onReorderTabs}
            lines={lineCount}
            size={activeFile?.size ?? 0}
            {commitBadge}
          />
          <div class="flex min-h-0 flex-1 flex-col overflow-hidden">
            {#if data.error}
              <div
                class="flex h-full items-center justify-center bg-surface-muted p-8 text-ui text-fg-secondary"
              >
                {data.error.message}
              </div>
            {:else if activeFile?.kind === 'text' && activeFile.content !== null}
              {#key `${activeFile.commitSha}:${activeFile.path}`}
                <FileViewer content={activeFile.content} />
              {/key}
            {:else if activeFile}
              <div
                class="flex h-full items-center justify-center bg-surface-muted p-8 text-ui text-fg-secondary"
              >
                {activeFile.path} cannot be previewed inline.
              </div>
            {:else}
              <div
                class="flex h-full items-center justify-center bg-surface-muted text-ui text-fg-subtle"
              >
                No file selected
              </div>
            {/if}
          </div>
        </div>
      </main>
    </div>
  </Sidebar.Root>
</div>
