<script lang="ts">
  import GitMerge from '~icons/lucide/git-merge';
  import { avatarPalette, formatFullDate, groupByDate, initials, timeAgo } from '@/commits-utils';
  import NavBar from '@/NavBar.svelte';
  import type { PageData } from './$types';

  const { data }: { data: PageData } = $props();

  const refName = $derived(data.view?.ref.name ?? 'main');
  const commitSha = $derived(data.view?.ref.commitSha ?? null);

  function hrefForCommit(sha: string): string {
    const query = new URLSearchParams();
    if (data.ref) query.set('ref', data.ref);
    const qs = query.toString();
    return `/${data.owner}/${data.repo}/commits/${sha}${qs.length > 0 ? `?${qs}` : ''}`;
  }

  function parseMergeTitle(title: string): { number: string; branch: string } | null {
    const match = title.match(/^Merge pull request #(\d+) from (.+)$/);
    if (!match || !match[1] || !match[2]) return null;
    return { number: match[1], branch: match[2] };
  }

  const groups = $derived(groupByDate(data.commits));
</script>

<svelte:head>
  <title>{data.owner}/{data.repo} · Commits</title>
</svelte:head>

<div class="grid h-full grid-rows-[42px_minmax(0,1fr)]">
  <NavBar
    owner={data.owner}
    repo={data.repo}
    {refName}
    {commitSha}
    page="commits"
    commitCount={data.commits.length > 0 ? data.commits.length : undefined}
  />

  <div class="overflow-hidden bg-canvas">
    <div class="flex h-full flex-col overflow-hidden">
      <main class="relative min-w-0 flex-1 overflow-hidden">
        <div
          class="relative grid h-full grid-rows-[auto_minmax(0,1fr)] overflow-hidden rounded-tl-ui rounded-tr-ui bg-surface-muted"
        >
          <!-- Header bar -->
          <div class="flex h-9.5 shrink-0 items-center justify-end bg-surface-muted px-4">
            {#if data.commits.length > 0}
              <span class="font-mono text-ui text-fg-muted">
                {data.commits.length} commit{data.commits.length !== 1 ? 's' : ''}
              </span>
            {/if}
          </div>

          <!-- Commits list -->
          <div class="overflow-y-auto">
            {#if data.error}
              <div class="flex h-full items-center justify-center p-8 text-ui text-fg-secondary">
                {data.error.message}
              </div>
            {:else if data.commits.length === 0}
              <div class="flex h-full items-center justify-center p-8 text-ui text-fg-subtle">
                No commits yet
              </div>
            {:else}
              <div class="py-2">
                {#each groups as group (group.label)}
                  <!-- Date group header -->
                  <div class="flex items-center gap-3 px-4 pb-1 pt-3">
                    <span class="whitespace-nowrap text-ui font-medium text-fg-subtle">
                      <span class="font-mono text-fg-muted">{group.commits.length}</span>
                      {group.label}
                    </span>
                    <div class="h-px flex-1 bg-line"></div>
                  </div>

                  <!-- Commits in group -->
                  {#each group.commits as commit (commit.sha)}
                    {@const isMerge = commit.parents.length >= 2}

                    {#if isMerge}
                      {@const pr = parseMergeTitle(commit.title)}
                      <!-- Merge card -->
                      <a
                        href={hrefForCommit(commit.sha)}
                        class="mx-2 my-0.5 block rounded-md border-l-2 border-accent bg-surface-accent px-3 py-2 outline-none transition-colors hover:bg-surface-accent/70 focus-visible:shadow-ring"
                      >
                        <div class="flex items-start gap-2">
                          <div
                            class="mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-avatar bg-accent"
                          >
                            <GitMerge class="text-bg" width={12} height={12} stroke-width={2.5} />
                          </div>
                          <div class="min-w-0 flex-1">
                            <div class="flex items-baseline gap-1.5 text-ui-md">
                              {#if pr}
                                <span class="font-medium text-fg">Pull request #{pr.number}</span>
                                <span class="truncate text-fg-secondary">{pr.branch}</span>
                              {:else}
                                <span class="font-medium text-fg">{commit.title}</span>
                              {/if}
                            </div>
                            <div class="mt-0.5 flex items-center gap-2 text-ui text-fg-muted">
                              <span>{commit.author.name}</span>
                              <span class="text-fg-slash">·</span>
                              <span title={formatFullDate(commit.committedAt)}
                                >{timeAgo(commit.committedAt)}</span
                              >
                              <span class="text-fg-slash">·</span>
                              <span class="font-mono text-fg-ref">{commit.sha.slice(0, 7)}</span>
                            </div>
                          </div>
                        </div>
                      </a>
                    {:else}
                      {@const palette = avatarPalette(commit.author.name)}
                      <!-- Regular commit row -->
                      <a
                        href={hrefForCommit(commit.sha)}
                        class="group flex h-8 items-center gap-3 px-4 outline-none hover:bg-overlay-hover focus-visible:shadow-ring"
                      >
                        <div
                          class="flex h-5 w-5 shrink-0 items-center justify-center rounded-avatar text-2xs font-semibold"
                          style="background-color: {palette.bg}; color: {palette.text};"
                        >
                          {initials(commit.author.name)}
                        </div>

                        <span class="min-w-0 flex-1 truncate text-ui-md text-fg">
                          {commit.title}
                        </span>

                        <span
                          class="flex w-20 shrink-0 items-center justify-end gap-1 font-mono text-ui opacity-0 transition-opacity duration-150 group-hover:opacity-100"
                        >
                          {#if commit.additions > 0}
                            <span class="text-diff-add-strong">+{commit.additions}</span>
                          {/if}
                          {#if commit.removals > 0}
                            <span class="text-danger">-{commit.removals}</span>
                          {/if}
                        </span>

                        <span class="shrink-0 text-ui text-fg-muted">
                          {commit.author.name}
                        </span>
                        <span
                          class="w-16 shrink-0 text-right text-ui text-fg-subtle"
                          title={formatFullDate(commit.committedAt)}
                        >
                          {timeAgo(commit.committedAt)}
                        </span>

                        <span class="w-20 shrink-0 text-right font-mono text-ui text-fg-ref">
                          {commit.sha.slice(0, 7)}
                        </span>
                      </a>
                    {/if}
                  {/each}
                {/each}
              </div>
            {/if}
          </div>
        </div>
      </main>
    </div>
  </div>
</div>
