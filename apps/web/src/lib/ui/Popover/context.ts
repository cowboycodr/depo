import { getContext, setContext } from 'svelte';

type PopoverContext = {
  visible: () => boolean;
  show: () => void;
  scheduleHide: () => void;
};

const key = Symbol('popover');

export const setPopoverContext = (ctx: PopoverContext) => setContext(key, ctx);

export const getPopoverContext = () => {
  const ctx = getContext<PopoverContext>(key);
  if (!ctx) throw new Error('Popover.Content must be rendered inside <Popover.Root>.');
  return ctx;
};
