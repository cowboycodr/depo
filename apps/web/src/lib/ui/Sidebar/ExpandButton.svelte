<script lang="ts">
  import PanelLeftOpen from '~icons/lucide/panel-left-open';
  import * as Button from '@/ui/Button';
  import { getSidebarContext } from '@/ui/Sidebar/context';

  const sidebar = getSidebarContext();
</script>

<Button.Root
  variant="sidebar-expand"
  label="Expand sidebar"
  title="Expand sidebar"
  onclick={() => sidebar.setOpen(true)}
>
  <PanelLeftOpen width={14} height={14} />
</Button.Root>
