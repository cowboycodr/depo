import { DepoApiError } from '@depo/api-client';
import { createDepoClient } from '@/server/depo-client';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ fetch, params, url }) => {
  const path = url.searchParams.get('path') ?? undefined;
  const ref = url.searchParams.get('ref') ?? undefined;
  const client = createDepoClient(fetch);

  try {
    return {
      owner: params.owner,
      repo: params.repo,
      ref: ref ?? null,
      path: path ?? null,
      view: await client.repos.view(params.owner, params.repo, { ref, path }),
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
