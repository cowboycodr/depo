import { getContext, setContext } from 'svelte';

export type CapsuleVariant = 'primary' | 'secondary';

type CapsuleContext = {
  variant: () => CapsuleVariant;
};

const key = Symbol('capsule');

export const setCapsuleContext = (context: CapsuleContext) => {
  setContext(key, context);
};

export const getCapsuleContext = () =>
  getContext<CapsuleContext>(key) ?? { variant: () => 'primary' };
