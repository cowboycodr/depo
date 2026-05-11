<script lang="ts">
  import { goto } from '$app/navigation';
  import { untrack } from 'svelte';
  import GitCommit from '~icons/lucide/git-commit';
  import type { FileDiff as ApiFileDiff, TreeEntry } from '@depo/api-client';
  import DiffViewer from '@/DiffViewer.svelte';
  import FileHeader from '@/FileHeader.svelte';
  import NavBar from '@/NavBar.svelte';
  import RepositoryTree from '@/RepositoryTree.svelte';
  import * as Sidebar from '@/ui/Sidebar';
  import { countLines, commitDate, reorderTabs } from '@/utils';
  import type { PageData } from './$types';

  const { data }: { data: PageData } = $props();

  let diffStyle: 'split' | 'unified' = $state('unified');
  let sidebarOpen = $state(true);
  let tabs = $state<string[]>([]);

  const files = $derived(data.commit?.diff.files ?? []);
  const treeNodes = $derived<TreeEntry[]>(
    files.map((f) => {
      const path = displayPath(f);
      return {
        path,
        name: path.split('/').pop() ?? path,
        kind: 'file',
        mode: f.newMode ?? f.oldMode ?? '100644',
        size: f.newFile.size ?? f.oldFile.size ?? 0,
        objectSha: f.newFile.objectSha ?? f.oldFile.objectSha ?? ''
      };
    })
  );
  const changedFilePaths = $derived(files.map((f) => displayPath(f)));
  const selectedFile = $derived(selectFile(files, data.file));
  const selectedPath = $derived(selectedFile ? displayPath(selectedFile) : null);
  const canRenderSelected = $derived(selectedFile ? canRenderFile(selectedFile) : false);
  const oldContent = $derived(selectedFile ? textContent(selectedFile.oldFile.content) : '');
  const newContent = $derived(selectedFile ? textContent(selectedFile.newFile.content) : '');
  const selectedLanguage = $derived(
    selectedFile?.newFile.language ?? selectedFile?.oldFile.language ?? undefined
  );
  const selectedSize = $derived(selectedFile?.newFile.size ?? selectedFile?.oldFile.size ?? 0);
  const selectedLines = $derived(countLines(newContent));

  $effect(() => {
    const path = selectedPath;
    if (path !== null) {
      untrack(() => {
        if (!tabs.includes(path)) {
          tabs = [...tabs, path];
        }
      });
    }
  });

  function selectFile(files: ApiFileDiff[], requestedPath: string | null): ApiFileDiff | null {
    if (files.length === 0) return null;
    if (requestedPath !== null) {
      const match = files.find((file) => displayPath(file) === requestedPath);
      if (match !== undefined) return match;
    }
    return files[0] ?? null;
  }

  function displayPath(file: ApiFileDiff): string {
    return file.newPath ?? file.oldPath ?? file.path;
  }

  function hrefForFile(file: ApiFileDiff): string {
    const query = new URLSearchParams({ file: displayPath(file) });
    return `?${query.toString()}`;
  }

  function hrefForPath(path: string): string {
    return `?${new URLSearchParams({ file: path }).toString()}`;
  }

  function canRenderFile(file: ApiFileDiff): boolean {
    const oldRenderable = file.oldFile.kind === 'text' || file.oldFile.kind === 'missing';
    const newRenderable = file.newFile.kind === 'text' || file.newFile.kind === 'missing';
    return oldRenderable && newRenderable && !file.binary;
  }

  function textContent(value: string | null): string {
    return value ?? '';
  }

  function shortSha(value: string): string {
    return value.slice(0, 7);
  }

  function closeTab(path: string) {
    const idx = tabs.indexOf(path);
    const next = tabs.filter((t) => t !== path);
    tabs = next;
    if (selectedPath === path) {
      const destination = next[idx] ?? next[idx - 1];
      if (destination) {
        const destFile = files.find((f) => displayPath(f) === destination);
        if (destFile) goto(hrefForFile(destFile));
      } else {
        const fallback = files.find((f) => displayPath(f) !== path);
        if (fallback) {
          goto(hrefForFile(fallback));
        } else {
          tabs = [path];
        }
      }
    }
  }

  function onReorderTabs(fromIndex: number, toIndex: number) {
    tabs = reorderTabs(tabs, fromIndex, toIndex);
  }
</script>

<svelte:head>
  <title>{data.owner}/{data.repo}@{shortSha(data.sha)} · Commit</title>
</svelte:head>

<div class="grid h-full grid-rows-[42px_minmax(0,1fr)]">
  <NavBar
    owner={data.owner}
    repo={data.repo}
    refName="commit"
    commitSha={data.commit?.commit.sha ?? data.sha}
    page="commits"
  />

  <Sidebar.Root bind:open={sidebarOpen}>
    <div class="flex h-[calc(100vh-42px)] overflow-hidden bg-canvas">
      <Sidebar.Panel>
        <div class="flex h-full flex-col overflow-hidden">
          {#if data.commit}
            <div class="border-b border-line px-3 py-3">
              <div class="flex items-center gap-2 text-ui-md font-medium text-fg">
                <GitCommit width={14} height={14} class="text-fg-ref" />
                <span class="min-w-0 truncate">{data.commit.commit.title}</span>
              </div>
              <div class="mt-1 flex items-center gap-2 font-mono text-ui text-fg-subtle">
                <span>{shortSha(data.commit.commit.sha)}</span>
                <span>{data.commit.commit.author.name}</span>
                <span>{commitDate(data.commit.commit.committedAt)}</span>
              </div>
            </div>

            <div
              class="flex items-center justify-between px-3 py-2 font-mono text-ui text-fg-muted"
            >
              <span>{data.commit.diff.stats.filesChanged} files</span>
              <span>
                <span class="text-diff-add-strong">+{data.commit.diff.stats.additions}</span>
                <span class="text-danger">-{data.commit.diff.stats.removals}</span>
              </span>
            </div>

            <div class="min-h-0 flex-1">
              <RepositoryTree
                nodes={treeNodes}
                selectedPath={selectedPath ?? undefined}
                changedPaths={changedFilePaths}
                {hrefForPath}
              />
            </div>
          {/if}
        </div>
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
            mode="diff"
            tabs={tabs.map((path) => {
              const file = files.find((f) => displayPath(f) === path);
              return { path, href: file ? hrefForFile(file) : '#' };
            })}
            activePath={selectedPath}
            onCloseTab={closeTab}
            {onReorderTabs}
            lines={selectedLines}
            size={selectedSize}
            additions={selectedFile?.additions ?? 0}
            removals={selectedFile?.removals ?? 0}
          />

          <div class="flex min-h-0 flex-1 flex-col overflow-hidden">
            {#if data.error}
              <div
                class="flex h-full items-center justify-center bg-surface-muted p-8 text-ui text-fg-secondary"
              >
                {data.error.message}
              </div>
            {:else if selectedFile && canRenderSelected}
              {#key `${selectedFile.oldFile.objectSha ?? 'missing'}:${selectedFile.newFile.objectSha ?? 'missing'}:${displayPath(selectedFile)}`}
                <DiffViewer
                  {diffStyle}
                  fileName={displayPath(selectedFile)}
                  language={selectedLanguage}
                  {oldContent}
                  {newContent}
                />
              {/key}
            {:else if selectedFile}
              <div
                class="flex h-full items-center justify-center bg-surface-muted p-8 text-ui text-fg-secondary"
              >
                {displayPath(selectedFile)} cannot be previewed inline.
              </div>
            {:else}
              <div
                class="flex h-full items-center justify-center bg-surface-muted text-ui text-fg-subtle"
              >
                No changed files
              </div>
            {/if}
          </div>
        </div>
      </main>
    </div>
  </Sidebar.Root>
</div>
