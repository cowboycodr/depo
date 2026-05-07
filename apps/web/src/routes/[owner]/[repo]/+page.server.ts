import { DepoApiError } from '@depo/api-client';
import { createDepoClient } from '@/server/depo-client';
import type { PageServerLoad } from './$types';

const README_NAMES = new Set(['README.md', 'readme.md', 'README', 'README.txt', 'readme.txt']);

export const load: PageServerLoad = async ({ fetch, params, url }) => {
  const path = url.searchParams.get('path') ?? undefined;
  const ref = url.searchParams.get('ref') ?? undefined;
  const client = createDepoClient(fetch);

  try {
    const view = await client.repos.view(params.owner, params.repo, { ref, path });

    const nofile = url.searchParams.get('nofile') === '1';

    if (!nofile && path === undefined && view.activeFile === null) {
      const readmePath = view.tree.nodes.find(
        (node) => node.kind === 'file' && README_NAMES.has(node.name)
      )?.path;
      if (readmePath !== undefined) {
        const viewWithReadme = await client.repos.view(params.owner, params.repo, {
          ref,
          path: readmePath
        });
        return {
          owner: params.owner,
          repo: params.repo,
          ref: ref ?? null,
          path: null,
          view: viewWithReadme,
          error: null
        };
      }
    }

    return {
      owner: params.owner,
      repo: params.repo,
      ref: ref ?? null,
      path: path ?? null,
      view,
      error: null
    };
  } catch (error) {
    if (error instanceof DepoApiError) {
      return {
        owner: params.owner,
        repo: params.repo,
        ref: ref ?? null,
        path: path ?? null,
        view: null,
        error: {
          code: error.code,
          message: error.message
        }
      };
    }

    throw error;
  }
};
