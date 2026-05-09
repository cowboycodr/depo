export type AppHref =
  | '/'
  | `/?${string}`
  | `/${string}/${string}`
  | `/${string}/${string}?${string}`
  | `/${string}/${string}/commits`
  | `/${string}/${string}/commits?${string}`
  | `/${string}/${string}/commits/${string}`
  | `/${string}/${string}/commits/${string}?${string}`;

export function appHref(path: string): AppHref {
  if (!path.startsWith('/')) {
    throw new Error(`App href must start with "/": ${path}`);
  }
  return path as AppHref;
}
