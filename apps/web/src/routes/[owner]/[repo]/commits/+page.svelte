<script lang="ts">
  import GitMerge from '~icons/lucide/git-merge';
  import type { CommitSummary } from '@depo/api-client';
  import { avatarPalette, formatFullDate, groupByDate, initials, timeAgo } from '@/commits-utils';
  import NavBar from '@/NavBar.svelte';
  import type { PageData } from './$types';

  const { data }: { data: PageData } = $props();

  const refName = $derived(data.view?.ref.name ?? 'main');
  const commitSha = $derived(data.view?.ref.commitSha ?? null);

  let expandedMerges: Record<string, boolean> = $state({});

  function toggleMerge(sha: string) {
    expandedMerges[sha] = !expandedMerges[sha];
  }

  function hrefForCommit(sha: string): string {
    const query = new URLSearchParams();
    if (data.ref) query.set('ref', data.ref);
    const qs = query.toString();
    return `/${data.owner}/${data.repo}/commits/${sha}${qs.length > 0 ? `?${qs}` : ''}`;
  }

  function mergeStats(contained: CommitSummary[]) {
    return contained.reduce(
      (acc, c) => ({
        additions: acc.additions + c.additions,
        removals: acc.removals + c.removals
      }),
      { additions: 0, removals: 0 }
    );
  }

  const groups = $derived(groupByDate(data.commits));
</script>

<svelte:head>
  <title>commits:{data.owner}/{data.repo}</title>
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
        <div class="relative h-full overflow-hidden rounded-tl-ui rounded-tr-ui bg-surface-muted">
          <!-- Commits list -->
          <div class="h-full overflow-y-auto">
            {#if data.error}
              <div class="flex h-full items-center justify-center p-8 text-ui text-fg-secondary">
                {data.error.message}
              </div>
            {:else if data.commits.length === 0}
              <div class="flex h-full items-center justify-center p-8 text-ui text-fg-subtle">
                No commits yet
              </div>
            {:else}
              {#snippet commitRow(c: CommitSummary)}
                {@const palette = avatarPalette(c.author.name)}
                <a
                  href={hrefForCommit(c.sha)}
                  class="group flex h-8 items-center gap-3 px-4 outline-none hover:bg-overlay-hover focus-visible:shadow-ring"
                >
                  <div
                    class="flex h-5 w-5 shrink-0 items-center justify-center rounded-avatar text-2xs font-semibold"
                    style="background-color: {palette.bg}; color: {palette.text};"
                  >
                    {initials(c.author.name)}
                  </div>

                  <span class="min-w-0 flex-1 truncate text-ui-md text-fg">
                    {c.title}
                  </span>

                  <span
                    class="flex w-20 shrink-0 items-center justify-end gap-1 font-mono text-ui opacity-0 transition-opacity duration-150 group-hover:opacity-100"
                  >
                    {#if c.additions > 0}
                      <span class="text-diff-add-strong">+{c.additions}</span>
                    {/if}
                    {#if c.removals > 0}
                      <span class="text-danger">-{c.removals}</span>
                    {/if}
                  </span>

                  <span class="shrink-0 text-ui text-fg-muted">
                    {c.author.name}
                  </span>
                  <span
                    class="w-16 shrink-0 text-right text-ui text-fg-subtle"
                    title={formatFullDate(c.committedAt)}
                  >
                    {timeAgo(c.committedAt)}
                  </span>

                  <span class="w-20 shrink-0 text-right font-mono text-ui text-fg-ref">
                    {c.sha.slice(0, 7)}
                  </span>
                </a>
              {/snippet}

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
                      {@const containedCount = commit.containedCommits?.length ?? 0}
                      {@const stats = mergeStats(commit.containedCommits ?? [])}
                      {@const expanded = expandedMerges[commit.sha] ?? false}
                      <div class="bg-overlay-hover">
                        <div
                          class="group flex h-8 items-center gap-3 px-4"
                          role="button"
                          tabindex="0"
                          onclick={() => toggleMerge(commit.sha)}
                          onkeydown={(e) => {
                            if (e.key === 'Enter' || e.key === ' ') toggleMerge(commit.sha);
                          }}
                        >
                          <div
                            class="flex h-5 w-5 shrink-0 items-center justify-center rounded-avatar bg-surface-chip"
                          >
                            <GitMerge
                              class="text-fg-muted"
                              width={12}
                              height={12}
                              stroke-width={2}
                            />
                          </div>

                          <span class="min-w-0 flex-1 truncate text-ui-md text-fg">
                            {commit.title}
                          </span>

                          {#if !expanded}
                            <span
                              class="flex shrink-0 items-center gap-1.5 text-ui opacity-0 transition-opacity duration-150 group-hover:opacity-100"
                            >
                              {#if containedCount > 0}
                                <span class="text-fg-muted">{containedCount} commits</span>
                              {/if}
                              {#if stats.additions > 0 || stats.removals > 0}
                                <span class="flex gap-1 font-mono">
                                  {#if stats.additions > 0}
                                    <span class="text-diff-add-strong">+{stats.additions}</span>
                                  {/if}
                                  {#if stats.removals > 0}
                                    <span class="text-danger">-{stats.removals}</span>
                                  {/if}
                                </span>
                              {/if}
                            </span>
                          {/if}

                          <span class="shrink-0 text-ui text-fg-muted">
                            {commit.author.name}
                          </span>
                          <span
                            class="w-16 shrink-0 text-right text-ui text-fg-subtle"
                            title={formatFullDate(commit.committedAt)}
                          >
                            {timeAgo(commit.committedAt)}
                          </span>

                          <a
                            href={hrefForCommit(commit.sha)}
                            class="w-20 shrink-0 text-right font-mono text-ui text-fg-ref outline-none hover:text-fg-bright"
                            onclick={(e) => e.stopPropagation()}
                          >
                            {commit.sha.slice(0, 7)}
                          </a>
                        </div>

                        {#if expanded}
                          {#each commit.containedCommits ?? [] as contained (contained.sha)}
                            {@render commitRow(contained)}
                          {/each}
                          <div class="flex h-8 items-center gap-2 px-4 text-ui text-fg-muted">
                            <span>{containedCount} commit{containedCount !== 1 ? 's' : ''}</span>
                            {#if stats.additions > 0 || stats.removals > 0}
                              <span class="text-fg-slash">·</span>
                              <span class="flex gap-1 font-mono">
                                {#if stats.additions > 0}
                                  <span class="text-diff-add-strong">+{stats.additions}</span>
                                {/if}
                                {#if stats.removals > 0}
                                  <span class="text-danger">-{stats.removals}</span>
                                {/if}
                              </span>
                            {/if}
                          </div>
                        {/if}
                      </div>
                    {:else}
                      {@render commitRow(commit)}
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
