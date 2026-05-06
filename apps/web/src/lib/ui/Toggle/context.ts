import { getContext, setContext } from 'svelte';

type ToggleContext = {
  value: () => string;
  setValue: (value: string) => void;
};

const key = Symbol('toggle');

export const setToggleContext = (context: ToggleContext) => {
  setContext(key, context);
};

export const getToggleContext = () => getContext<ToggleContext>(key);
