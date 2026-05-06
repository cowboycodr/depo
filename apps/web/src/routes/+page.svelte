<script lang="ts">
  import AlertTriangle from '~icons/lucide/alert-triangle';
  import ArrowRight from '~icons/lucide/arrow-right';
  import GitBranch from '~icons/lucide/git-branch';
  import Plus from '~icons/lucide/plus';
  import type { ActionData, PageData } from './$types';

  const { data, form }: { data: PageData; form: ActionData } = $props();

  const values = $derived(form?.values);
  const owner = $derived(values?.owner ?? '');
  const name = $derived(values?.name ?? '');
  const defaultBranch = $derived(values?.defaultBranch ?? 'main');
  const authorName = $derived(values?.authorName ?? 'Depo');
  const authorEmail = $derived(values?.authorEmail ?? 'depo@localhost');
  const readme = $derived(values?.readme ?? '');

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
    <div class="mx-auto grid max-w-6xl gap-5 lg:grid-cols-[390px_minmax(0,1fr)]">
      <section class="rounded-ui bg-surface-muted p-4 shadow-ring">
        <div class="mb-3 flex items-center justify-between">
          <h1 class="text-ui-md font-medium text-fg-bright">Create repository</h1>
          <span class="font-mono text-ui-xs text-fg-ref">README.md</span>
        </div>

        {#if form?.error}
          <div class="mb-3 flex items-start gap-2 rounded-ui bg-surface px-3 py-2 text-ui text-danger">
            <AlertTriangle class="mt-0.5 flex-none" width={13} height={13} stroke-width={2} />
            <span>{form.error}</span>
          </div>
        {/if}

        {#if data.error}
          <div class="mb-3 flex items-start gap-2 rounded-ui bg-surface px-3 py-2 text-ui text-danger">
            <AlertTriangle class="mt-0.5 flex-none" width={13} height={13} stroke-width={2} />
            <span>{data.error.message}</span>
          </div>
        {/if}

        <form method="POST" class="space-y-3">
          <div class="grid gap-2 sm:grid-cols-2 lg:grid-cols-1 xl:grid-cols-2">
            <label class="block">
              <span class="mb-1 block text-ui text-fg-secondary">Owner</span>
              <input
                class="h-8 w-full rounded-ui border border-border bg-bg px-2.5 font-mono text-ui-md text-fg outline-none focus:shadow-ring"
                name="owner"
                value={owner}
                autocomplete="off"
                spellcheck="false"
                required
              />
            </label>

            <label class="block">
              <span class="mb-1 block text-ui text-fg-secondary">Repository</span>
              <input
                class="h-8 w-full rounded-ui border border-border bg-bg px-2.5 font-mono text-ui-md text-fg outline-none focus:shadow-ring"
                name="name"
                value={name}
                autocomplete="off"
                spellcheck="false"
                required
              />
            </label>
          </div>

          <label class="block">
            <span class="mb-1 block text-ui text-fg-secondary">Branch</span>
            <input
              class="h-8 w-full rounded-ui border border-border bg-bg px-2.5 font-mono text-ui-md text-fg outline-none focus:shadow-ring"
              name="defaultBranch"
              value={defaultBranch}
              autocomplete="off"
              spellcheck="false"
              required
            />
          </label>

          <div class="grid gap-2 sm:grid-cols-2 lg:grid-cols-1 xl:grid-cols-2">
            <label class="block">
              <span class="mb-1 block text-ui text-fg-secondary">Author</span>
              <input
                class="h-8 w-full rounded-ui border border-border bg-bg px-2.5 text-ui-md text-fg outline-none focus:shadow-ring"
                name="authorName"
                value={authorName}
                autocomplete="name"
                required
              />
            </label>

            <label class="block">
              <span class="mb-1 block text-ui text-fg-secondary">Email</span>
              <input
                class="h-8 w-full rounded-ui border border-border bg-bg px-2.5 text-ui-md text-fg outline-none focus:shadow-ring"
                name="authorEmail"
                type="email"
                value={authorEmail}
                autocomplete="email"
                required
              />
            </label>
          </div>

          <label class="block">
            <span class="mb-1 block text-ui text-fg-secondary">README.md</span>
            <textarea
              class="min-h-34 w-full resize-y rounded-ui border border-border bg-bg px-2.5 py-2 font-mono text-code leading-code text-fg outline-none focus:shadow-ring"
              name="readme"
              spellcheck="false"
              placeholder="# repository"
              value={readme}
            ></textarea>
          </label>

          <button
            class="inline-flex h-8 w-full items-center justify-center gap-2 rounded-ui bg-surface-inverse px-3 text-ui-md font-medium text-bg transition-colors hover:opacity-90 focus-visible:shadow-ring"
            type="submit"
          >
            <Plus width={13} height={13} stroke-width={2} />
            <span>Create</span>
          </button>
        </form>
      </section>

      <section class="min-w-0 rounded-ui bg-surface-muted p-4 shadow-ring">
        <div class="mb-3 flex items-center justify-between">
          <h2 class="text-ui-md font-medium text-fg-bright">Repositories</h2>
          <span class="font-mono text-ui-xs text-fg-ref">{data.repos.length}</span>
        </div>

        {#if data.repos.length > 0}
          <div class="grid gap-1.5">
            {#each data.repos as repo (repo.id)}
              <a
                class="grid min-h-11 grid-cols-[minmax(0,1fr)_auto] items-center gap-3 rounded-ui bg-bg px-3 py-2 text-left shadow-ring transition-colors hover:bg-surface-hover"
                href={repoHref(repo.owner, repo.name)}
              >
                <span class="min-w-0">
                  <span class="block truncate text-ui-md font-medium text-fg-bright">
                    {repo.owner}<span class="text-fg-slash">/</span>{repo.name}
                  </span>
                  <span class="mt-1 inline-flex items-center gap-1.5 font-mono text-ui-xs text-fg-ref">
                    <GitBranch width={11} height={11} stroke-width={2} />
                    {repo.defaultBranch}
                  </span>
                </span>
                <ArrowRight class="text-fg-tertiary" width={14} height={14} stroke-width={2} />
              </a>
            {/each}
          </div>
        {:else}
          <div class="flex h-48 items-center justify-center rounded-ui bg-bg text-ui text-fg-secondary shadow-ring">
            No repositories yet.
          </div>
        {/if}
      </section>
    </div>
  </main>
</div>
