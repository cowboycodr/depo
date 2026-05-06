import { fail, redirect } from '@sveltejs/kit';
import { DepoApiError } from '@depo/api-client';
import { createDepoClient } from '@/server/depo-client';
import type { Actions, PageServerLoad } from './$types';

type FormValues = {
  owner: string;
  name: string;
  defaultBranch: string;
  authorName: string;
  authorEmail: string;
  readme: string;
};

export const load: PageServerLoad = async ({ fetch }) => {
  const client = createDepoClient(fetch);

  try {
    return {
      repos: await client.repos.list(),
      error: null
    };
  } catch (error) {
    if (error instanceof DepoApiError) {
      return {
        repos: [],
        error: {
          code: error.code,
          message: error.message
        }
      };
    }

    throw error;
  }
};

export const actions = {
  default: async ({ request, fetch }) => {
    const values = formValues(await request.formData());
    const missing = requiredFields.filter(([key]) => values[key].trim().length === 0);

    if (missing.length > 0) {
      return fail(400, {
        values,
        error: `${missing.map(([, label]) => label).join(', ')} required.`
      });
    }

    const client = createDepoClient(fetch);
    const branch = values.defaultBranch.trim();
    const readme = values.readme.trim().length > 0 ? values.readme : defaultReadme(values.name);

    try {
      const repo = await client.repos.create({
        owner: values.owner.trim(),
        name: values.name.trim(),
        defaultBranch: branch
      });

      await repo.createCommit({
        targetBranch: branch,
        message: 'Initial commit',
        author: {
          name: values.authorName.trim(),
          email: values.authorEmail.trim()
        },
        changes: [
          {
            type: 'upsertText',
            path: 'README.md',
            content: ensureTrailingNewline(readme)
          }
        ]
      });
    } catch (error) {
      if (error instanceof DepoApiError) {
        return fail(error.status >= 500 ? 502 : error.status, {
          values,
          error: error.message
        });
      }

      throw error;
    }

    redirect(
      303,
      `/${encodeURIComponent(values.owner.trim())}/${encodeURIComponent(values.name.trim())}?path=README.md`
    );
  }
} satisfies Actions;

const requiredFields: Array<[keyof FormValues, string]> = [
  ['owner', 'Owner'],
  ['name', 'Repository'],
  ['defaultBranch', 'Branch'],
  ['authorName', 'Author name'],
  ['authorEmail', 'Author email']
];

function formValues(formData: FormData): FormValues {
  return {
    owner: field(formData, 'owner'),
    name: field(formData, 'name'),
    defaultBranch: field(formData, 'defaultBranch'),
    authorName: field(formData, 'authorName'),
    authorEmail: field(formData, 'authorEmail'),
    readme: field(formData, 'readme')
  };
}

function field(formData: FormData, key: string) {
  const value = formData.get(key);
  return typeof value === 'string' ? value : '';
}

function defaultReadme(name: string) {
  return `# ${name.trim()}\n\n`;
}

function ensureTrailingNewline(value: string) {
  return value.endsWith('\n') ? value : `${value}\n`;
}
