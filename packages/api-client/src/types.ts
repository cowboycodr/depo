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

export type CommitSummary = {
  sha: string;
  title: string;
  author: CommitAuthor;
  committedAt: string;
};

export type CommitListResponse = {
  commits: CommitSummary[];
};

export type DiffStats = {
  filesChanged: number;
  additions: number;
  removals: number;
};

export type DiffFileStatus =
  | "added"
  | "modified"
  | "deleted"
  | "renamed"
  | "copied"
  | "typeChanged"
  | "unknown";

export type DiffContentKind = "text" | "binary" | "tooLarge" | "missing" | "unloaded";

export type DiffFileContent = {
  path: string | null;
  kind: DiffContentKind;
  language: string | null;
  mode: string | null;
  size: number | null;
  encoding: string | null;
  content: string | null;
  objectSha: string | null;
};

export type FileDiff = {
  path: string;
  oldPath: string | null;
  newPath: string | null;
  status: DiffFileStatus;
  oldMode: string | null;
  newMode: string | null;
  additions: number;
  removals: number;
  binary: boolean;
  oldFile: DiffFileContent;
  newFile: DiffFileContent;
};

export type CommitDiff = {
  baseSha: string | null;
  headSha: string;
  stats: DiffStats;
  files: FileDiff[];
};

export type CommitDetail = {
  sha: string;
  treeSha: string;
  parents: string[];
  author: CommitAuthor;
  authoredAt: string;
  committer: CommitAuthor;
  committedAt: string;
  title: string;
  message: string;
};

export type CommitDetailResponse = {
  repo: Repository;
  commit: CommitDetail;
  diff: CommitDiff;
};

export type DiffResponse = {
  repo: Repository;
  diff: CommitDiff;
};

export type CommitDetailParams = {
  path?: string | null;
};

export type DiffParams = {
  base?: string | null;
  head: string;
  path?: string | null;
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
