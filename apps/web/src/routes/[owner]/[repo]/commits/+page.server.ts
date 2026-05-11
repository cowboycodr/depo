import { DepoApiError } from '@depo/api-client';
import { createDepoClient } from '@/server/depo-client';
import { extractApiError } from '@/server/load-utils';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ fetch, params, url }) => {
  const ref = url.searchParams.get('ref') ?? undefined;
  const client = createDepoClient(fetch);

  try {
    const [view, commitList] = await Promise.all([
      client.repos.view(params.owner, params.repo, { ref }),
      client.repos.commits(params.owner, params.repo, { ref, limit: 100 })
    ]);

    return {
      owner: params.owner,
      repo: params.repo,
      ref: ref ?? null,
      view,
      commits: commitList.commits,
      error: null
    };
  } catch (error) {
    const apiError = extractApiError(error);
    if (apiError) {
      return {
        owner: params.owner,
        repo: params.repo,
        ref: ref ?? null,
        view: null,
        commits: [],
        error: apiError
      };
    }
    throw error;
  }
};
