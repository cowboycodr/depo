<script lang="ts">
  import type { Land } from '@depo/api-client';
  import NavBar from '@/NavBar.svelte';
  import { formatFullDate, timeAgo } from '@/commits-utils';
  import type { PageData } from './$types';

  const { data }: { data: PageData } = $props();

  const zeroSha = '0000000000000000000000000000000000000000';

  function landVerb(land: Land): string {
    if (land.kind === 'branch_created') return 'created';
    if (land.kind === 'branch_deleted') return 'deleted';
    return 'landed';
  }

  function landNoun(land: Land): string {
    if (land.kind === 'branch_deleted') return land.shortRef;
    const count = land.commitCount;
    return `${count} commit${count === 1 ? '' : 's'} on ${land.shortRef}`;
  }

  function hrefForLand(land: Land): string {
    if (land.newSha !== zeroSha) {
      return `/${data.owner}/${data.repo}/commits/${land.newSha}`;
    }
    return `/${data.owner}/${data.repo}/code`;
  }

  function statusClass(status: Land['status']): string {
    if (status === 'passed') return 'text-diff-add-strong';
    if (status === 'failed') return 'text-danger';
    if (status === 'checking') return 'text-fg-secondary';
    return 'text-fg-muted';
  }
</script>

<svelte:head>
  <title>{data.owner}/{data.repo}</title>
  <meta name="description" content={`Depo lands feed for ${data.owner}/${data.repo}.`} />
</svelte:head>

<div class="grid h-full grid-rows-[42px_minmax(0,1fr)]">
  <NavBar
    owner={data.owner}
    repo={data.repo}
    refName={data.repository?.defaultBranch ?? 'main'}
    commitSha={null}
    page="lands"
  />

  <div class="overflow-hidden bg-canvas">
    <main class="relative h-full min-w-0 overflow-hidden">
      <div class="relative h-full overflow-hidden rounded-tl-ui rounded-tr-ui bg-surface-muted">
        <div class="h-full overflow-y-auto">
          {#if data.error}
            <div class="flex h-full items-center justify-center p-8 text-ui text-fg-secondary">
              {data.error.message}
            </div>
          {:else if data.lands.length === 0}
            <div class="flex h-full items-center justify-center p-8 text-ui text-fg-subtle">
              No lands yet
            </div>
          {:else}
            <div class="py-2">
              {#each data.lands as land (land.id)}
                <a
                  href={hrefForLand(land)}
                  class="group grid h-10 grid-cols-[minmax(0,1fr)_auto_auto_auto] items-center gap-4 px-4 outline-none hover:bg-overlay-hover focus-visible:shadow-ring"
                >
                  <div class="min-w-0">
                    <div class="flex min-w-0 items-center gap-1.5 text-ui-md">
                      <span class="shrink-0 text-fg-muted">{land.actor}</span>
                      <span class="shrink-0 text-fg-subtle">{landVerb(land)}</span>
                      <span class="min-w-0 truncate text-fg">{landNoun(land)}</span>
                    </div>
                    {#if land.headTitle}
                      <div class="truncate text-ui text-fg-muted">{land.headTitle}</div>
                    {/if}
                  </div>

                  <div class="hidden w-24 justify-end gap-1 font-mono text-ui sm:flex">
                    {#if land.additions > 0}
                      <span class="text-diff-add-strong">+{land.additions}</span>
                    {/if}
                    {#if land.removals > 0}
                      <span class="text-danger">-{land.removals}</span>
                    {/if}
                  </div>

                  <span class={['w-18 text-right text-ui', statusClass(land.status)].join(' ')}>
                    {land.status}
                  </span>

                  <span
                    class="w-16 text-right text-ui text-fg-subtle"
                    title={formatFullDate(land.pushedAt)}
                  >
                    {timeAgo(land.pushedAt)}
                  </span>
                </a>
              {/each}
            </div>
          {/if}
        </div>
      </div>
    </main>
  </div>
</div>
