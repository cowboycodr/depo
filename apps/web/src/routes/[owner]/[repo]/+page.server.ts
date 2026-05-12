import { createDepoClient } from '@/server/depo-client';
import { extractApiError } from '@/server/load-utils';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ fetch, params }) => {
  const client = createDepoClient(fetch);

  try {
    const [repository, landList] = await Promise.all([
      client.repos.get(params.owner, params.repo),
      client.repos.lands(params.owner, params.repo, { limit: 50 })
    ]);

    return {
      owner: params.owner,
      repo: params.repo,
      repository: repository.repo,
      lands: landList.lands,
      error: null
    };
  } catch (error) {
    const apiError = extractApiError(error);
    if (apiError) {
      return {
        owner: params.owner,
        repo: params.repo,
        repository: null,
        lands: [],
        error: apiError
      };
    }
    throw error;
  }
};
