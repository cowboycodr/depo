<script lang="ts">
  import AlertTriangle from '~icons/lucide/alert-triangle';
  import ArrowRight from '~icons/lucide/arrow-right';
  import GitBranch from '~icons/lucide/git-branch';
  import type { ActionData, PageData } from './$types';

  const { data, form }: { data: PageData; form: ActionData } = $props();

  const owner = $derived(form?.values?.owner ?? '');
  const name = $derived(form?.values?.name ?? '');

  const repoHref = (owner: string, name: string) =>
    `/${encodeURIComponent(owner)}/${encodeURIComponent(name)}`;
</script>

<svelte:head>
  <title>Depo</title>
  <meta name="description" content="Depo repositories" />
</svelte:head>

<div class="grid h-full min-h-0 grid-rows-[42px_minmax(0,1fr)] bg-bg text-fg">
  <header class="flex h-10.5 shrink-0 items-center border-b border-line px-4">
    <a href="/" class="flex items-center gap-2 text-ui-md font-medium text-fg-bright">
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
            id="depo-home"
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
          stroke="url(#depo-home)"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
      <span>Depo</span>
    </a>
  </header>

  <main class="min-h-0 overflow-y-auto bg-canvas px-5 py-5">
    <div class="mx-auto grid max-w-5xl gap-4 lg:grid-cols-[260px_minmax(0,1fr)]">
      <section class="rounded-ui bg-surface-muted p-4 shadow-ring">
        <h1 class="mb-3.5 text-ui-md font-medium text-fg-bright">New repository</h1>

        {#if form?.error}
          <div
            class="mb-3 flex items-start gap-1.5 rounded-ui bg-bg px-2.5 py-2 text-ui text-danger shadow-ring"
          >
            <AlertTriangle class="mt-px flex-none" width={12} height={12} stroke-width={2} />
            <span>{form.error}</span>
          </div>
        {/if}

        <form method="POST" class="space-y-2">
          <div
            class="flex h-8 items-center overflow-hidden rounded-ui border border-border bg-bg font-mono text-ui-md focus-within:shadow-ring"
          >
            <input
              class="h-full min-w-0 flex-1 bg-transparent pl-2.5 pr-1 text-fg outline-none placeholder:text-fg-muted"
              name="owner"
              value={owner}
              placeholder="owner"
              autocomplete="off"
              spellcheck="false"
              required
            />
            <span class="select-none text-fg-slash">/</span>
            <input
              class="h-full min-w-0 flex-1 bg-transparent pl-1 pr-2.5 text-fg outline-none placeholder:text-fg-muted"
              name="name"
              value={name}
              placeholder="name"
              autocomplete="off"
              spellcheck="false"
              required
            />
          </div>

          <button
            class="inline-flex h-8 w-full items-center justify-center gap-1.5 rounded-ui bg-surface-inverse px-3 text-ui-md font-medium text-bg transition-opacity hover:opacity-90 focus-visible:shadow-ring"
            type="submit"
          >
            Create
            <ArrowRight width={13} height={13} stroke-width={2} />
          </button>
        </form>
      </section>

      <section class="rounded-ui bg-surface-muted p-4 shadow-ring">
        <div class="mb-3 flex items-center justify-between">
          <h2 class="text-ui-md font-medium text-fg-bright">Repositories</h2>
          <span class="font-mono text-ui-xs text-fg-ref">{data.repos.length}</span>
        </div>

        {#if data.error}
          <div
            class="mb-3 flex items-start gap-1.5 rounded-ui bg-bg px-2.5 py-2 text-ui text-danger shadow-ring"
          >
            <AlertTriangle class="mt-px flex-none" width={12} height={12} stroke-width={2} />
            <span>{data.error.message}</span>
          </div>
        {/if}

        {#if data.repos.length > 0}
          <div class="grid gap-1.5">
            {#each data.repos as repo (repo.id)}
              <a
                class="grid min-h-11 grid-cols-[minmax(0,1fr)_auto] items-center gap-3 rounded-ui bg-bg px-3 py-2 shadow-ring transition-colors hover:bg-surface-hover"
                href={repoHref(repo.owner, repo.name)}
              >
                <span class="min-w-0">
                  <span class="block truncate font-mono text-ui-md font-medium text-fg-bright">
                    {repo.owner}<span class="text-fg-slash">/</span>{repo.name}
                  </span>
                  <span
                    class="mt-0.5 inline-flex items-center gap-1.5 font-mono text-ui-xs text-fg-ref"
                  >
                    <GitBranch width={11} height={11} stroke-width={2} />
                    {repo.defaultBranch}
                  </span>
                </span>
                <ArrowRight class="text-fg-tertiary" width={14} height={14} stroke-width={2} />
              </a>
            {/each}
          </div>
        {:else}
          <div
            class="flex h-32 items-center justify-center rounded-ui bg-bg text-ui text-fg-secondary shadow-ring"
          >
            No repositories yet.
          </div>
        {/if}
      </section>
    </div>
  </main>
</div>
