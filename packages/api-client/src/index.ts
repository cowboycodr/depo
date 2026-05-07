export type {
  BlobResponse,
  CommitAuthor,
  CommitDetail,
  CommitDetailParams,
  CommitDetailResponse,
  CommitDiff,
  CommitChange,
  DiffContentKind,
  DiffFileContent,
  DiffFileStatus,
  DiffParams,
  DiffResponse,
  DiffStats,
  FileDiff,
  CommitListResponse,
  CommitResponse,
  CommitSummary,
  CreateCommitInput,
  CreateRepositoryInput,
  DepoErrorBody,
  ReadParams,
  Repository,
  RepositoryListResponse,
  RepositoryResponse,
  RepositoryView,
  TreeEntry,
  TreeResponse,
} from "./types.js";

import type {
  BlobResponse,
  CommitDetailParams,
  CommitDetailResponse,
  CommitListResponse,
  CommitResponse,
  CreateCommitInput,
  CreateRepositoryInput,
  DepoErrorBody,
  DiffParams,
  DiffResponse,
  ReadParams,
  Repository,
  RepositoryListResponse,
  RepositoryResponse,
  RepositoryView,
  TreeResponse,
} from "./types.js";

export type DepoClientOptions = {
  baseUrl: string;
  token?: string;
  fetchImpl?: typeof fetch;
};

export class DepoApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly details: unknown;

  constructor(status: number, body: DepoErrorBody) {
    super(body.message);
    this.name = "DepoApiError";
    this.status = status;
    this.code = body.code;
    this.details = body.details;
  }
}

export class DepoClient {
  readonly repos: RepositoriesResource;
  readonly baseUrl: string;
  readonly token: string | undefined;
  private readonly fetchImpl: typeof fetch;

  constructor(options: DepoClientOptions) {
    this.baseUrl = options.baseUrl.replace(/\/+$/, "");
    this.token = options.token;
    this.fetchImpl = options.fetchImpl ?? fetch;
    this.repos = new RepositoriesResource(this);
  }

  async request<T>(method: string, path: string, body?: unknown): Promise<T> {
    const headers = new Headers();
    if (this.token !== undefined) {
      headers.set("authorization", `Bearer ${this.token}`);
    }
    if (body !== undefined) {
      headers.set("content-type", "application/json");
    }

    const init: RequestInit = {
      method,
      headers,
    };
    if (body !== undefined) {
      init.body = JSON.stringify(body);
    }

    const response = await this.fetchImpl(`${this.baseUrl}${path}`, init);

    const text = await response.text();
    const data = text.length > 0 ? JSON.parse(text) : undefined;

    if (!response.ok) {
      const error = data?.error as DepoErrorBody | undefined;
      throw new DepoApiError(response.status, error ?? {
        code: "unknown_error",
        message: `Depo request failed with HTTP ${response.status}`,
        details: null,
      });
    }

    return data as T;
  }
}

export class RepositoriesResource {
  private readonly client: DepoClient;

  constructor(client: DepoClient) {
    this.client = client;
  }

  async create(input: CreateRepositoryInput): Promise<DepoRepository> {
    const response = await this.client.request<RepositoryResponse>(
      "POST",
      "/api/v1/repos",
      input,
    );
    return new DepoRepository(this.client, response.repo);
  }

  async list(): Promise<Repository[]> {
    const response = await this.client.request<RepositoryListResponse>(
      "GET",
      "/api/v1/repos",
    );
    return response.repos;
  }

  async get(owner: string, repo: string): Promise<DepoRepository> {
    const response = await this.client.request<RepositoryResponse>(
      "GET",
      repoPath(owner, repo),
    );
    return new DepoRepository(this.client, response.repo);
  }

  async view(owner: string, repo: string, params: ReadParams = {}): Promise<RepositoryView> {
    return this.client.request<RepositoryView>(
      "GET",
      `${repoPath(owner, repo)}/view${readQuery(params)}`,
    );
  }

  async commits(
    owner: string,
    repo: string,
    params: ReadParams & { limit?: number } = {},
  ): Promise<CommitListResponse> {
    const query = new URLSearchParams();
    if (params.ref !== undefined) query.set("ref", params.ref);
    if (params.limit !== undefined) query.set("limit", String(params.limit));
    const qs = query.toString();
    return this.client.request<CommitListResponse>(
      "GET",
      `${repoPath(owner, repo)}/commits${qs.length > 0 ? `?${qs}` : ""}`,
    );
  }

  async commit(
    owner: string,
    repo: string,
    sha: string,
    params: CommitDetailParams = {},
  ): Promise<CommitDetailResponse> {
    const query = new URLSearchParams();
    if (params.path !== undefined && params.path !== null) query.set("path", params.path);
    const qs = query.toString();
    return this.client.request<CommitDetailResponse>(
      "GET",
      `${repoPath(owner, repo)}/commits/${encodeURIComponent(sha)}${qs.length > 0 ? `?${qs}` : ""}`,
    );
  }

  async diff(owner: string, repo: string, params: DiffParams): Promise<DiffResponse> {
    const query = new URLSearchParams();
    if (params.base !== undefined && params.base !== null) query.set("base", params.base);
    query.set("head", params.head);
    if (params.path !== undefined && params.path !== null) query.set("path", params.path);
    return this.client.request<DiffResponse>(
      "GET",
      `${repoPath(owner, repo)}/diff?${query.toString()}`,
    );
  }
}

export class DepoRepository {
  readonly repo: Repository;
  private readonly client: DepoClient;

  constructor(client: DepoClient, repo: Repository) {
    this.client = client;
    this.repo = repo;
  }

  async createCommit(input: CreateCommitInput): Promise<CommitResponse> {
    return this.client.request<CommitResponse>(
      "POST",
      `${repoPath(this.repo.owner, this.repo.name)}/commits`,
      input,
    );
  }

  async tree(params: ReadParams = {}): Promise<TreeResponse> {
    return this.client.request<TreeResponse>(
      "GET",
      `${repoPath(this.repo.owner, this.repo.name)}/tree${readQuery(params)}`,
    );
  }

  async blob(params: ReadParams): Promise<BlobResponse> {
    return this.client.request<BlobResponse>(
      "GET",
      `${repoPath(this.repo.owner, this.repo.name)}/blob${readQuery(params)}`,
    );
  }

  async view(params: ReadParams = {}): Promise<RepositoryView> {
    return this.client.request<RepositoryView>(
      "GET",
      `${repoPath(this.repo.owner, this.repo.name)}/view${readQuery(params)}`,
    );
  }
}

function repoPath(owner: string, repo: string): string {
  return `/api/v1/repos/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}`;
}

function readQuery(params: ReadParams): string {
  const query = new URLSearchParams();
  if (params.ref !== undefined) {
    query.set("ref", params.ref);
  }
  if (params.path !== undefined) {
    query.set("path", params.path);
  }
  const value = query.toString();
  return value.length > 0 ? `?${value}` : "";
}
