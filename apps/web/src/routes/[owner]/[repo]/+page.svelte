<script lang="ts">
  import { goto } from '$app/navigation';
  import { resolve } from '$app/paths';
  import { untrack } from 'svelte';
  import type { PageData } from './$types';
  import FileHeader from '@/FileHeader.svelte';
  import FileViewer from '@/FileViewer.svelte';
  import NavBar from '@/NavBar.svelte';
  import RepositoryTree from '@/RepositoryTree.svelte';
  import { appHref, type AppHref } from '@/navigation';
  import * as Sidebar from '@/ui/Sidebar';

  const { data }: { data: PageData } = $props();

  let diffStyle: 'split' | 'unified' = $state('unified');
  let sidebarOpen = $state(true);
  let tabs = $state<string[]>([]);

  const activeFile = $derived(data.view?.activeFile ?? null);
  const activePath = $derived(activeFile?.path ?? data.path ?? null);
  const content = $derived(activeFile?.content ?? '');
  const lineCount = $derived(countLines(content));
  const fileCount = $derived(
    data.view?.tree.nodes.filter((node) => node.kind === 'file').length ?? 0
  );
  const fileCountLabel = $derived(fileCount === 1 ? '1 file' : `${fileCount} files`);
  const commitBadge = $derived(
    activeFile?.lastCommit
      ? {
          sha: activeFile.lastCommit.sha,
          title: activeFile.lastCommit.title,
          href: appHref(`/${data.owner}/${data.repo}/commits/${activeFile.lastCommit.sha}`),
          author: activeFile.lastCommit.author,
          committedAt: activeFile.lastCommit.committedAt,
          additions: activeFile.lastCommit.additions,
          removals: activeFile.lastCommit.removals,
          description: activeFile.lastCommit.description
        }
      : undefined
  );

  const hrefForPath = (path: string): AppHref => {
    const query = new URLSearchParams();
    if (data.ref) query.set('ref', data.ref);
    query.set('path', path);
    return appHref(`/${data.owner}/${data.repo}?${query.toString()}`);
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
        goto(resolve(hrefForPath(destination)));
      } else {
        const query = new URLSearchParams();
        if (data.ref) query.set('ref', data.ref);
        query.set('nofile', '1');
        goto(resolve(appHref(`/${data.owner}/${data.repo}?${query.toString()}`)));
      }
    }
  }

  function reorderTabs(fromIndex: number, toIndex: number) {
    const next = [...tabs];
    const [item] = next.splice(fromIndex, 1) as [string];
    next.splice(toIndex, 0, item);
    tabs = next;
  }

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
    page="code"
    commitCount={data.view?.recentCommits.length}
  />

  <Sidebar.Root bind:open={sidebarOpen}>
    <div class="flex h-[calc(100vh-42px)] overflow-hidden bg-canvas">
      <Sidebar.Panel>
        {#snippet controls()}
          <div class="flex min-w-0 flex-1 items-center justify-between gap-2 pl-1.5">
            <div class="flex min-w-0 items-baseline gap-2">
              <span class="font-mono text-ui-md font-medium text-fg-secondary">Files</span>
              <span class="font-mono text-ui-xs text-fg-ref">{fileCountLabel}</span>
            </div>
            <Sidebar.CollapseButton />
          </div>
        {/snippet}

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
            onReorderTabs={reorderTabs}
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
