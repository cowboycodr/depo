<script lang="ts">
  import PanelLeftClose from '~icons/lucide/panel-left-close';
  import * as Button from '@/ui/Button';
  import { getSidebarContext } from '@/ui/Sidebar/context';

  const sidebar = getSidebarContext();
</script>

<Button.Root
  variant="sidebar-collapse"
  label="Collapse sidebar"
  title="Collapse sidebar"
  onclick={() => sidebar.setOpen(false)}
>
  <PanelLeftClose width={15} height={15} />
</Button.Root>
