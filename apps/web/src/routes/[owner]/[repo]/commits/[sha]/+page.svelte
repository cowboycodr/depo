<script lang="ts">
  import GitCommit from '~icons/lucide/git-commit';
  import type { FileDiff as ApiFileDiff } from '@depo/api-client';
  import DiffViewer from '@/DiffViewer.svelte';
  import FileHeader from '@/FileHeader.svelte';
  import LanguageIcon from '@/LanguageIcon.svelte';
  import NavBar from '@/NavBar.svelte';
  import * as Sidebar from '@/ui/Sidebar';
  import type { PageData } from './$types';

  const { data }: { data: PageData } = $props();

  let diffStyle: 'split' | 'unified' = $state('unified');
  let sidebarOpen = $state(true);

  const files = $derived(data.commit?.diff.files ?? []);
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

  function canRenderFile(file: ApiFileDiff): boolean {
    const oldRenderable = file.oldFile.kind === 'text' || file.oldFile.kind === 'missing';
    const newRenderable = file.newFile.kind === 'text' || file.newFile.kind === 'missing';
    return oldRenderable && newRenderable && !file.binary;
  }

  function textContent(value: string | null): string {
    return value ?? '';
  }

  function countLines(value: string): number {
    if (value.length === 0) return 0;
    return value.endsWith('\n') ? value.slice(0, -1).split('\n').length : value.split('\n').length;
  }

  function shortSha(value: string): string {
    return value.slice(0, 7);
  }

  function commitDate(value: string): string {
    return new Date(value).toLocaleString('en', {
      month: 'short',
      day: 'numeric',
      hour: 'numeric',
      minute: '2-digit'
    });
  }

  function statusLabel(status: ApiFileDiff['status']): string {
    switch (status) {
      case 'added':
        return 'A';
      case 'modified':
        return 'M';
      case 'deleted':
        return 'D';
      case 'renamed':
        return 'R';
      case 'copied':
        return 'C';
      case 'typeChanged':
        return 'T';
      default:
        return '?';
    }
  }

  function statusClass(status: ApiFileDiff['status']): string {
    if (status === 'added' || status === 'copied') return 'text-diff-add-strong';
    if (status === 'deleted') return 'text-danger';
    if (status === 'renamed') return 'text-fg-ref';
    return 'text-fg-muted';
  }

  function noop() {}

  function reorderNoop(_fromIndex: number, _toIndex: number) {}
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

            <div class="flex items-center justify-between px-3 py-2 font-mono text-ui text-fg-muted">
              <span>{data.commit.diff.stats.filesChanged} files</span>
              <span>
                <span class="text-diff-add-strong">+{data.commit.diff.stats.additions}</span>
                <span class="text-danger">-{data.commit.diff.stats.removals}</span>
              </span>
            </div>

            <div class="min-h-0 flex-1 overflow-y-auto px-2 pb-3">
              <div class="space-y-0.5">
                {#each data.commit.diff.files as file (displayPath(file))}
                  {@const active = selectedFile === file}
                  <a
                    href={hrefForFile(file)}
                    class={[
                      'group grid h-[25px] grid-cols-[1fr_auto] items-center gap-2 rounded-avatar px-2 text-ui-md outline-none focus-visible:shadow-ring',
                      active
                        ? 'bg-surface-hover font-medium text-fg'
                        : 'text-fg-secondary hover:bg-overlay-hover hover:text-fg'
                    ].join(' ')}
                    aria-current={active ? 'page' : undefined}
                  >
                    <span class="flex min-w-0 items-center gap-1.5">
                      <LanguageIcon name={displayPath(file)} />
                      <span class="min-w-0 truncate">{displayPath(file)}</span>
                    </span>
                    <span class="flex shrink-0 items-center gap-1.5 font-mono text-ui-xs">
                      <span class={statusClass(file.status)}>{statusLabel(file.status)}</span>
                      {#if file.additions > 0}
                        <span class="text-diff-add-strong">+{file.additions}</span>
                      {/if}
                      {#if file.removals > 0}
                        <span class="text-danger">-{file.removals}</span>
                      {/if}
                    </span>
                  </a>
                {/each}
              </div>
            </div>
          {/if}
        </div>
      </Sidebar.Panel>

      <main class="relative min-w-0 flex-1 overflow-hidden">
        <div
          class={[
            'relative grid h-full grid-rows-[auto_auto_minmax(0,1fr)] overflow-hidden rounded-tl-ui bg-surface-muted [transition:border-top-right-radius_200ms_ease-in-out]',
            !sidebarOpen ? 'rounded-tr-ui' : ''
          ].join(' ')}
        >
          <div class="flex h-9.5 shrink-0 items-center justify-between gap-4 bg-surface-muted px-4">
            {#if data.commit}
              <div class="flex min-w-0 items-center gap-2">
                <GitCommit width={14} height={14} class="shrink-0 text-fg-ref" />
                <span class="min-w-0 truncate text-ui-md font-medium text-fg">
                  {data.commit.commit.title}
                </span>
              </div>
              <div class="flex shrink-0 items-center gap-2 font-mono text-ui text-fg-muted">
                <span>{shortSha(data.commit.commit.sha)}</span>
                <span class="text-diff-add-strong">+{data.commit.diff.stats.additions}</span>
                <span class="text-danger">-{data.commit.diff.stats.removals}</span>
              </div>
            {:else}
              <span class="text-ui text-fg-secondary">Commit</span>
            {/if}
          </div>

          <FileHeader
            bind:diffStyle
            mode="diff"
            tabs={selectedPath ? [{ path: selectedPath, href: selectedFile ? hrefForFile(selectedFile) : '?' }] : []}
            activePath={selectedPath}
            onCloseTab={noop}
            onReorderTabs={reorderNoop}
            lines={selectedLines}
            size={selectedSize}
            additions={selectedFile?.additions ?? 0}
            removals={selectedFile?.removals ?? 0}
          />

          <div class="flex min-h-0 flex-1 flex-col overflow-hidden">
            {#if data.error}
              <div class="flex h-full items-center justify-center bg-surface-muted p-8 text-ui text-fg-secondary">
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
              <div class="flex h-full items-center justify-center bg-surface-muted p-8 text-ui text-fg-secondary">
                {displayPath(selectedFile)} cannot be previewed inline.
              </div>
            {:else}
              <div class="flex h-full items-center justify-center bg-surface-muted text-ui text-fg-subtle">
                No changed files
              </div>
            {/if}
          </div>
        </div>
      </main>
    </div>
  </Sidebar.Root>
</div>
