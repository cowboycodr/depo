<script lang="ts">
  import Search from '~icons/lucide/search';
  import * as Button from '@/ui/Button';
  import * as Capsule from '@/ui/Capsule';

  const {
    owner,
    repo,
    refName = 'main',
    commitSha,
    page = 'lands',
    commitCount: _commitCount
  }: {
    owner: string;
    repo: string;
    refName?: string;
    commitSha?: string | null;
    page?: 'lands' | 'code' | 'commits';
    commitCount?: number;
  } = $props();

  const tabs = $derived([
    { label: 'Lands', page: 'lands', href: `/${owner}/${repo}` },
    { label: 'Code', page: 'code', href: `/${owner}/${repo}/code` },
    { label: 'Commits', page: 'commits', href: `/${owner}/${repo}/commits` },
    { label: 'Pull Requests', page: 'prs', disabled: true },
    { label: 'Issues', page: 'issues', disabled: true }
  ]);
</script>

<nav class="flex h-10.5 shrink-0 items-stretch gap-0 p-0 pr-1.75">
  <!-- Logo -->
  <div
    class="flex w-12.5 flex-none items-center justify-center self-stretch"
    aria-label="Pacific Code"
  >
    <svg
      class="block h-4 w-4.5 flex-none"
      width="18"
      height="16"
      viewBox="0 0 18 16"
      fill="none"
      aria-hidden="true"
    >
      <defs>
        <linearGradient
          id="pacificcode"
          x1="13"
          y1="8"
          x2="9"
          y2="15"
          gradientUnits="userSpaceOnUse"
        >
          <stop offset="0" stop-color="var(--prism-pink)" />
          <stop offset="0.28" stop-color="var(--prism-yellow)" />
          <stop offset="0.58" stop-color="var(--prism-green)" />
          <stop offset="1" stop-color="var(--prism-cyan)" />
        </linearGradient>
      </defs>
      <polygon
        points="9,1 17,15 1,15"
        class="fill-none stroke-logo-stroke stroke-[1.1]"
        stroke-linejoin="round"
      />
      <path
        d="M13 8 L17 15 L9 15"
        class="fill-none stroke-[1.1]"
        stroke="url(#pacificcode)"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </svg>
  </div>

  <Capsule.Root>
    <Capsule.Extension side="left" class="pl-[2.5px]">
      <img class="block h-6 w-6 rounded-avatar" src="/pacificcode.svg" alt="Repository owner" />
      <span class="pl-1.25 text-fg-bright">{owner}</span>
      <span class="text-fg-slash">/</span>
      <span>{repo}</span>
    </Capsule.Extension>

    <Capsule.Core>
      {#each tabs as tab (tab.label)}
        <Capsule.Tab
          active={tab.page === page}
          disabled={tab.disabled}
          href={tab.disabled ? undefined : tab.href}
        >
          {tab.label}
        </Capsule.Tab>
      {/each}
    </Capsule.Core>

    <Capsule.Extension side="right">
      <span class="text-fg-secondary">{refName}</span>
      <span class="text-fg-ref">{commitSha ? commitSha.slice(0, 7) : 'empty'}</span>
    </Capsule.Extension>
  </Capsule.Root>

  <!-- Actions -->
  <div class="ml-auto flex items-center">
    <div class="flex items-center gap-1.25">
      <Button.Root label="Search" class="min-w-33 cursor-default justify-start">
        <Search class="flex-none text-fg-tertiary" width={12} height={12} stroke-width={2} />
        <span class="text-ui">Search</span>
      </Button.Root>
    </div>
  </div>
</nav>
