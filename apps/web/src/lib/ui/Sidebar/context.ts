import { getContext, setContext } from 'svelte';

type SidebarContext = {
  open: () => boolean;
  setOpen: (open: boolean) => void;
};

const key = Symbol('sidebar');

export const setSidebarContext = (context: SidebarContext) => {
  setContext(key, context);
};

export const getSidebarContext = () => {
  const context = getContext<SidebarContext>(key);

  if (!context) {
    throw new Error('Sidebar components must be rendered inside <Sidebar.Root>.');
  }

  return context;
};
