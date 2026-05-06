import type { TreeEntry } from '@depo/api-client';

export type TreeNode = {
  name: string;
  path: string;
  type: 'file' | 'folder';
  children: TreeNode[];
};

export type VisibleNode = {
  node: TreeNode;
  depth: number;
  displayName: string;
};

export const pathsFromEntries = (entries: TreeEntry[]) =>
  entries.map((entry) => (entry.kind === 'directory' ? `${entry.path}/` : entry.path));

export const createTree = (filePaths: string[]) => {
  const root: TreeNode = { name: '', path: '', type: 'folder', children: [] };

  for (const filePath of filePaths) {
    const isFolder = filePath.endsWith('/');
    const segments = filePath.replace(/\/$/, '').split('/');
    let current = root;
    let currentPath = '';

    segments.forEach((segment, index) => {
      currentPath = currentPath ? `${currentPath}/${segment}` : segment;
      const isLeaf = index === segments.length - 1;
      const type = isLeaf && !isFolder ? 'file' : 'folder';
      let next = current.children.find((child) => child.name === segment);

      if (!next) {
        next = { name: segment, path: currentPath, type, children: [] };
        current.children.push(next);
      }

      current = next;
    });
  }

  sortTree(root);
  return root;
};

export const ancestorsForPath = (path: string) =>
  path
    .split('/')
    .slice(0, -1)
    .reduce<string[]>((ancestors, segment) => {
      const previous = ancestors.at(-1);
      ancestors.push(previous ? `${previous}/${segment}` : segment);
      return ancestors;
    }, []);

export const compressFolder = (node: TreeNode, changedPaths: Set<string>) => {
  let current = node;
  const names = [current.name];

  while (
    current.children.length === 1 &&
    current.children[0]?.type === 'folder' &&
    !changedPaths.has(current.path)
  ) {
    current = current.children[0];
    names.push(current.name);
  }

  return { node: current, displayName: names.join(' / ') };
};

export const getVisibleRows = (
  tree: TreeNode,
  isOpen: (path: string) => boolean,
  changedPaths: Set<string>
) => {
  const rows: VisibleNode[] = [];

  const walk = (nodes: TreeNode[], depth: number) => {
    for (const child of nodes) {
      if (child.type === 'folder') {
        const compressed = compressFolder(child, changedPaths);
        rows.push({ ...compressed, depth });

        if (isOpen(compressed.node.path)) {
          walk(compressed.node.children, depth + 1);
        }
      } else {
        rows.push({ node: child, depth, displayName: child.name });
      }
    }
  };

  walk(tree.children, 0);
  return rows;
};

export const hasChangedDescendant = (node: TreeNode, changedPaths: Set<string>): boolean =>
  changedPaths.has(node.path) ||
  node.children.some((child) => hasChangedDescendant(child, changedPaths));

const compareNodes = (a: TreeNode, b: TreeNode) => {
  if (a.type !== b.type) return a.type === 'folder' ? -1 : 1;
  return a.name.localeCompare(b.name, undefined, { sensitivity: 'base' });
};

const sortTree = (node: TreeNode) => {
  node.children.sort(compareNodes);
  node.children.forEach(sortTree);
};
