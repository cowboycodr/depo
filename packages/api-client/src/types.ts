export type Repository = {
  id: string;
  owner: string;
  name: string;
  defaultBranch: string;
  createdAt: string;
  updatedAt: string;
};

export type CreateRepositoryInput = {
  owner: string;
  name: string;
  defaultBranch?: string;
};

export type RepositoryResponse = {
  repo: Repository;
};

export type RepositoryListResponse = {
  repos: Repository[];
  nextCursor: string | null;
  hasMore: boolean;
};

export type CommitAuthor = {
  name: string;
  email: string;
};

export type CommitChange =
  | {
      type: "upsert";
      path: string;
      contentBase64: string;
      mode?: "100644" | "100755";
    }
  | {
      type: "upsertText";
      path: string;
      content: string;
      mode?: "100644" | "100755";
    };

export type CreateCommitInput = {
  targetBranch: string;
  expectedHeadSha?: string | null;
  message: string;
  author: CommitAuthor;
  changes: CommitChange[];
};

export type CommitResponse = {
  commit: {
    sha: string;
    treeSha: string;
    branch: string;
  };
  refUpdate: {
    oldSha: string;
    newSha: string;
    status: string;
  };
};

export type TreeEntry = {
  path: string;
  name: string;
  kind: "file" | "directory";
  mode: string;
  size: number;
  objectSha: string;
};

export type TreeResponse = {
  path: string;
  commitSha: string;
  nodes: TreeEntry[];
};

export type BlobResponse = {
  path: string;
  kind: "text" | "binary" | "tooLarge";
  language: string | null;
  mode: string;
  size: number;
  encoding: string | null;
  content: string | null;
  commitSha: string;
  objectSha: string;
  etag: string;
};

export type RepositoryView = {
  repo: Repository;
  ref: {
    name: string;
    kind: "branch" | "commit";
    commitSha: string | null;
  };
  branches: {
    defaultBranch: string;
    items: Array<{
      name: string;
      headSha: string;
    }>;
  };
  tree: {
    nodes: TreeEntry[];
  };
  activeFile: BlobResponse | null;
  recentCommits: Array<{
    sha: string;
    title: string;
    author: CommitAuthor;
    committedAt: string;
  }>;
};

export type ReadParams = {
  ref?: string;
  path?: string;
};

export type DepoErrorBody = {
  code: string;
  message: string;
  details: unknown;
};
