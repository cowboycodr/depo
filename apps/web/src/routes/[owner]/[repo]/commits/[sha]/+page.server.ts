import { DepoApiError } from '@depo/api-client';
import { createDepoClient } from '@/server/depo-client';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ fetch, params, url }) => {
  const file = url.searchParams.get('file') ?? null;
  const client = createDepoClient(fetch);

  try {
    const commit = await client.repos.commit(params.owner, params.repo, params.sha, {
      path: file
    });

    return {
      owner: params.owner,
      repo: params.repo,
      sha: params.sha,
      file,
      commit,
      error: null
    };
  } catch (error) {
    if (error instanceof DepoApiError) {
      return {
        owner: params.owner,
        repo: params.repo,
        sha: params.sha,
        file,
        commit: null,
        error: {
          code: error.code,
          message: error.message
        }
      };
    }

    throw error;
  }
};
