<script lang="ts">
  import { resolve } from '$app/paths';
  import type { CommitSummary } from '@depo/api-client';
  import NavBar from '@/NavBar.svelte';
  import { appHref, type AppHref } from '@/navigation';
  import type { PageData } from './$types';

  const { data }: { data: PageData } = $props();

  const refName = $derived(data.view?.ref.name ?? 'main');
  const commitSha = $derived(data.view?.ref.commitSha ?? null);

  type CommitGroup = { label: string; commits: CommitSummary[] };

  function groupLabel(isoDate: string): string {
    const date = new Date(isoDate);
    const now = new Date();
    const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const d = new Date(date.getFullYear(), date.getMonth(), date.getDate());
    const diff = Math.round((today.getTime() - d.getTime()) / 86400000);
    if (diff === 0) return 'Today';
    if (diff === 1) return 'Yesterday';
    if (diff < 7) return d.toLocaleDateString('en', { weekday: 'long' });
    if (d.getFullYear() === now.getFullYear())
      return d.toLocaleDateString('en', { month: 'long', day: 'numeric' });
    return d.toLocaleDateString('en', { month: 'long', day: 'numeric', year: 'numeric' });
  }

  function groupByDate(commits: CommitSummary[]): CommitGroup[] {
    const groups: CommitGroup[] = [];
    let currentLabel = '';
    for (const commit of commits) {
      const label = groupLabel(commit.committedAt);
      if (label !== currentLabel) {
        currentLabel = label;
        groups.push({ label, commits: [commit] });
      } else {
        const last = groups[groups.length - 1];
        if (last) last.commits.push(commit);
      }
    }
    return groups;
  }

  function formatFullDate(isoDate: string): string {
    return new Date(isoDate).toLocaleString(undefined, {
      year: 'numeric',
      month: 'long',
      day: 'numeric',
      hour: 'numeric',
      minute: '2-digit',
      second: '2-digit'
    });
  }

  function timeAgo(isoDate: string): string {
    const ms = Date.now() - new Date(isoDate).getTime();
    const s = Math.floor(ms / 1000);
    const m = Math.floor(s / 60);
    const h = Math.floor(m / 60);
    const d = Math.floor(h / 24);
    if (s < 60) return 'just now';
    if (m < 60) return `${m}m ago`;
    if (h < 24) return `${h}h ago`;
    if (d < 30) return `${d}d ago`;
    if (d < 365) return `${Math.floor(d / 30)}mo ago`;
    return `${Math.floor(d / 365)}y ago`;
  }

  type Palette = { bg: string; text: string };

  const AVATAR_PALETTES: Palette[] = [
    { bg: '#1a2e20', text: '#4ade80' },
    { bg: '#1a2035', text: '#60a5fa' },
    { bg: '#26183a', text: '#c084fc' },
    { bg: '#301a26', text: '#f472b6' },
    { bg: '#2e2018', text: '#fb923c' },
    { bg: '#272e18', text: '#a3e635' }
  ];

  function avatarPalette(name: string): Palette {
    let h = 0;
    for (let i = 0; i < name.length; i++) h = (Math.imul(31, h) + name.charCodeAt(i)) | 0;
    return AVATAR_PALETTES[Math.abs(h) % AVATAR_PALETTES.length] ?? AVATAR_PALETTES[0]!;
  }

  function initials(name: string): string {
    const parts = name.trim().split(/\s+/).filter(Boolean);
    const first = parts[0] ?? '';
    const last = parts.length > 1 ? (parts[parts.length - 1] ?? '') : '';
    if (parts.length <= 1) return (first[0] ?? '?').toUpperCase();
    return ((first[0] ?? '') + (last[0] ?? '')).toUpperCase();
  }

  function hrefForCommit(sha: string): AppHref {
    const query = new URLSearchParams();
    if (data.ref) query.set('ref', data.ref);
    const qs = query.toString();
    return appHref(`/${data.owner}/${data.repo}/commits/${sha}${qs.length > 0 ? `?${qs}` : ''}`);
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
                    {@const palette = avatarPalette(commit.author.name)}
                    <a
                      href={resolve(hrefForCommit(commit.sha))}
                      class="group flex h-8 items-center gap-3 px-4 outline-none hover:bg-overlay-hover focus-visible:shadow-ring"
                    >
                      <!-- Author avatar -->
                      <div
                        class="flex h-5 w-5 shrink-0 items-center justify-center rounded-avatar text-2xs font-semibold"
                        style="background-color: {palette.bg}; color: {palette.text};"
                      >
                        {initials(commit.author.name)}
                      </div>

                      <!-- Commit title -->
                      <span class="min-w-0 flex-1 truncate text-ui-md text-fg">
                        {commit.title}
                      </span>

                      <!-- Changes -->
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

                      <!-- Author + time -->
                      <span class="shrink-0 text-ui text-fg-muted">
                        {commit.author.name}
                      </span>
                      <span
                        class="w-16 shrink-0 text-right text-ui text-fg-subtle"
                        title={formatFullDate(commit.committedAt)}
                      >
                        {timeAgo(commit.committedAt)}
                      </span>

                      <!-- SHA -->
                      <span class="w-20 shrink-0 text-right font-mono text-ui text-fg-ref">
                        {commit.sha.slice(0, 7)}
                      </span>
                    </a>
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
