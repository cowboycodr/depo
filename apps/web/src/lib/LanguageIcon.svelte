<script lang="ts">
  import CssIcon from '~icons/material-icon-theme/css';
  import DocumentIcon from '~icons/material-icon-theme/document';
  import GitIcon from '~icons/material-icon-theme/git';
  import JavascriptIcon from '~icons/material-icon-theme/javascript';
  import JsonIcon from '~icons/material-icon-theme/json';
  import MarkdownIcon from '~icons/material-icon-theme/markdown';
  import NpmIcon from '~icons/material-icon-theme/npm';
  import ReactIcon from '~icons/material-icon-theme/react';
  import ReactTsIcon from '~icons/material-icon-theme/react-ts';
  import ReadmeIcon from '~icons/material-icon-theme/readme';
  import TypescriptIcon from '~icons/material-icon-theme/typescript';

  const { name, label = undefined }: { name: string; label?: string } = $props();

  const fileIcons = {
    css: { label: 'CSS', component: CssIcon },
    git: { label: 'Git', component: GitIcon },
    js: { label: 'JavaScript', component: JavascriptIcon },
    jsx: { label: 'React', component: ReactIcon },
    json: { label: 'JSON', component: JsonIcon },
    md: { label: 'Markdown', component: MarkdownIcon },
    npm: { label: 'npm', component: NpmIcon },
    readme: { label: 'README', component: ReadmeIcon },
    ts: { label: 'TypeScript', component: TypescriptIcon },
    tsx: { label: 'TypeScript React', component: ReactTsIcon }
  } as const;

  const extensionFor = (fileName: string) => fileName.split('.').pop()?.toLowerCase() ?? '';
  const iconKeyFor = (fileName: string) => {
    const normalized = fileName.toLowerCase();

    if (normalized === 'readme.md') return 'readme';
    if (normalized === 'package.json') return 'npm';
    if (normalized.startsWith('.git')) return 'git';

    return extensionFor(fileName);
  };

  const iconKey = iconKeyFor(name);
  const icon = $derived(iconKey && (iconKey in fileIcons) ? fileIcons[iconKey as keyof typeof fileIcons] : undefined);
  const fallback = $derived((label ?? extensionFor(name) ?? 'file').slice(0, 4));
</script>

{#if icon}
  <icon.component class="block h-3.75 w-3.75 shrink-0" aria-label={icon.label} />
{:else}
  <DocumentIcon class="block h-3.75 w-3.75 shrink-0" aria-label={fallback} />
{/if}
