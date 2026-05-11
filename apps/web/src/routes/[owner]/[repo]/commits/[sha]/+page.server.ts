import { DepoApiError } from '@depo/api-client';
import { createDepoClient } from '@/server/depo-client';
import { extractApiError } from '@/server/load-utils';
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
    const apiError = extractApiError(error);
    if (apiError) {
      return {
        owner: params.owner,
        repo: params.repo,
        sha: params.sha,
        file,
        commit: null,
        error: apiError
      };
    }
    throw error;
  }
};
