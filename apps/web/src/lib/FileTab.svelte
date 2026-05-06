<script lang="ts">
  import LanguageIcon from '@/LanguageIcon.svelte';
  import * as Capsule from '@/ui/Capsule';

  const { fileName }: { fileName: string } = $props();

  const lastDot = $derived(fileName.lastIndexOf('.'));
  const base = $derived(lastDot > 0 ? fileName.slice(0, lastDot) : fileName);
  const ext = $derived(lastDot > 0 ? fileName.slice(lastDot) : '');
</script>

<Capsule.Tab active>
  <LanguageIcon name={fileName} />
  <span class="truncate font-medium pl-1">
    {base}<span class="font-normal text-fg-secondary">{ext}</span>
  </span>
</Capsule.Tab>
