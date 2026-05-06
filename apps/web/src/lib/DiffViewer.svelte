<script lang="ts">
  import { onMount } from 'svelte';
  import type { FileContents, FileDiff } from '@pierre/diffs';
  import type { ThemeRegistration } from 'shiki';
  import darkTheme from '@/themes/depo-dark.json';
  import lightTheme from '@/themes/depo-light.json';

  const {
    diffStyle = 'unified',
    fileName = 'README.md',
    language,
    content = ''
  }: {
    diffStyle?: 'split' | 'unified';
    fileName?: string;
    language?: string;
    content?: string;
  } = $props();

  let diffMount: HTMLDivElement | undefined = $state();
  let diff: FileDiff | undefined = $state();
  let mounted = $state(false);

  const normalizeLanguage = (name: string, lang: string | undefined) => {
    if (lang === 'markdown') return 'md';
    if (lang) return lang;
    return name.includes('.') ? name.split('.').pop() || 'txt' : 'txt';
  };

  const fileLanguage = $derived(normalizeLanguage(fileName, language));
  const oldFile = $derived<FileContents>({ name: fileName, lang: fileLanguage, contents: '' });
  const newFile = $derived<FileContents>({ name: fileName, lang: fileLanguage, contents: content });

  const applyOptions = () => {
    diff?.setOptions({
      collapsed: false,
      diffIndicators: 'bars',
      diffStyle,
      disableFileHeader: true,
      hunkSeparators: 'line-info',
      lineDiffType: 'word-alt',
      overflow: 'scroll',
      theme: { dark: 'depo-dark', light: 'depo-light' },
      themeType: 'system'
    });
  };

  const renderDiff = () => {
    diff?.render({ containerWrapper: diffMount!, forceRender: true, newFile, oldFile });
  };

  onMount(() => {
    let cancelled = false;

    void import('@pierre/diffs').then(({ FileDiff, registerCustomTheme }) => {
      if (cancelled) return;

      registerCustomTheme('depo-dark', async () => darkTheme as ThemeRegistration);
      registerCustomTheme('depo-light', async () => lightTheme as ThemeRegistration);

      diff = new FileDiff();
      mounted = true;
    });

    return () => {
      cancelled = true;
      diff?.cleanUp();
      diff = undefined;
    };
  });

  $effect(() => {
    if (mounted) {
      applyOptions();
      renderDiff();
    }
  });
</script>

<div
  bind:this={diffMount}
  class="h-full w-full min-w-0 overflow-auto bg-surface-muted font-mono text-code leading-code"
></div>
