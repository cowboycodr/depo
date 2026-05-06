<script lang="ts">
  const {
    content = ''
  }: {
    content?: string;
  } = $props();

  const lines = $derived(splitLines(content));

  function splitLines(value: string) {
    if (value.length === 0) return [''];
    const normalized = value.endsWith('\n') ? value.slice(0, -1) : value;
    return normalized.split('\n');
  }
</script>

<div class="h-full w-full min-w-0 overflow-auto bg-surface-muted font-mono text-code leading-code">
  <div class="min-w-max py-2">
    {#each lines as line, index}
      <div class="grid grid-cols-[4.25rem_minmax(0,1fr)] px-0">
        <span class="select-none pr-4 text-right text-fg-ref">{index + 1}</span>
        <code class="whitespace-pre pr-6 text-fg">{line || ' '}</code>
      </div>
    {/each}
  </div>
</div>
