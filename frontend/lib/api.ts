const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080"

type RequestOptions = {
  method?: string
  body?: unknown
  headers?: Record<string, string>
}

class ApiClient {
  private baseUrl: string
  private token: string | null = null

  constructor(baseUrl: string) {
    this.baseUrl = baseUrl
    if (typeof window !== "undefined") {
      this.token = localStorage.getItem("auth_token")
    }
  }

  setToken(token: string | null) {
    this.token = token
    if (typeof window !== "undefined") {
      if (token) {
        localStorage.setItem("auth_token", token)
      } else {
        localStorage.removeItem("auth_token")
      }
    }
  }

  getToken() {
    if (typeof window !== "undefined" && !this.token) {
      this.token = localStorage.getItem("auth_token")
    }
    return this.token
  }

  private async request<T>(endpoint: string, options: RequestOptions = {}): Promise<T> {
    const { method = "GET", body, headers = {} } = options

    const token = this.getToken()
    if (token) {
      headers["Authorization"] = `Bearer ${token}`
    }

    if (body) {
      headers["Content-Type"] = "application/json"
    }

    const response = await fetch(`${this.baseUrl}${endpoint}`, {
      method,
      headers,
      body: body ? JSON.stringify(body) : undefined,
    })

    if (!response.ok) {
      const error = await response.json().catch(() => ({ message: "Request failed" }))
      throw new Error(error.message || `HTTP error ${response.status}`)
    }

    return response.json()
  }

  // Auth endpoints
  async register(data: { email: string; name: string; password: string }) {
    const response = await this.request<AuthResponse>("/v1/auth/register", {
      method: "POST",
      body: data,
    })
    this.setToken(response.token)
    return response
  }

  async login(data: { email: string; password: string }) {
    const response = await this.request<AuthResponse>("/v1/auth/login", {
      method: "POST",
      body: data,
    })
    this.setToken(response.token)
    return response
  }

  getGitHubOAuthUrl() {
    return `${this.baseUrl}/v1/auth/github`
  }

  async getMe() {
    return this.request<User>("/v1/auth/me")
  }

  logout() {
    this.setToken(null)
  }

  // Projects endpoints
  async getProjects() {
    return this.request<Project[]>("/v1/projects")
  }

  async getProject(id: string) {
    return this.request<Project>(`/v1/projects/${id}`)
  }

  async createProject(data: CreateProjectRequest) {
    return this.request<Project>("/v1/projects", {
      method: "POST",
      body: data,
    })
  }

  async updateProject(id: string, data: UpdateProjectRequest) {
    return this.request<Project>(`/v1/projects/${id}`, {
      method: "PATCH",
      body: data,
    })
  }

  async deleteProject(id: string) {
    return this.request<{ deleted: boolean }>(`/v1/projects/${id}`, {
      method: "DELETE",
    })
  }

  // Environment variables
  async getEnvVars(projectId: string) {
    return this.request<EnvVarEntry[]>(`/v1/projects/${projectId}/env`)
  }

  async setEnvVars(projectId: string, envVars: EnvVarEntry[]) {
    return this.request<EnvVarEntry[]>(`/v1/projects/${projectId}/env`, {
      method: "PUT",
      body: { env_vars: envVars },
    })
  }

  async addEnvVar(projectId: string, data: { key: string; value: string; target?: EnvVarTarget }) {
    return this.request<EnvVarEntry[]>(`/v1/projects/${projectId}/env`, {
      method: "POST",
      body: data,
    })
  }

  async deleteEnvVar(projectId: string, key: string) {
    return this.request<EnvVarEntry[]>(`/v1/projects/${projectId}/env/${key}`, {
      method: "DELETE",
    })
  }

  // GitHub linking
  async linkGitHub(projectId: string, data: { github_repo: string; installation_id: number }) {
    return this.request<Project>(`/v1/projects/${projectId}/link`, {
      method: "POST",
      body: data,
    })
  }

  // GitHub
  async getGitHubRepos() {
    return this.request<GitHubRepo[]>("/v1/github/repos")
  }

  // Deployments
  async getDeployments() {
    return this.request<Deployment[]>("/v1/deployments")
  }

  async getProjectDeployments(projectId: string) {
    return this.request<Deployment[]>(`/v1/projects/${projectId}/deployments`)
  }

  async getDeployment(id: string) {
    return this.request<Deployment>(`/v1/deployments/${id}`)
  }

  async createDeployment(projectId: string, data: CreateDeploymentRequest) {
    return this.request<Deployment>(`/v1/projects/${projectId}/deployments`, {
      method: "POST",
      body: data,
    })
  }

  async cancelDeployment(id: string) {
    return this.request<{ cancelled: boolean }>(`/v1/deployments/${id}/cancel`, {
      method: "POST",
    })
  }

  async promoteDeployment(id: string) {
    return this.request<{ promoted: boolean }>(`/v1/deployments/${id}/promote`, {
      method: "POST",
    })
  }

  streamDeploymentLogs(id: string, onMessage: (log: string) => void, onError?: (error: Event) => void) {
    const token = this.getToken()
    const eventSource = new EventSource(
      `${this.baseUrl}/v1/deployments/${id}/logs${token ? `?token=${token}` : ""}`
    )

    eventSource.onmessage = (event) => {
      onMessage(event.data)
    }

    eventSource.onerror = (error) => {
      onError?.(error)
      eventSource.close()
    }

    return () => eventSource.close()
  }

  // API Keys
  async getApiKeys() {
    return this.request<ApiKey[]>("/v1/api-keys")
  }

  async createApiKey(data: { name: string; expires_in_days?: number }) {
    return this.request<ApiKey & { key_plain: string }>("/v1/api-keys", {
      method: "POST",
      body: data,
    })
  }

  async revokeApiKey(id: string) {
    return this.request<{ revoked: boolean }>(`/v1/api-keys/${id}`, {
      method: "DELETE",
    })
  }
}

export const api = new ApiClient(API_BASE_URL)

// Types
export interface User {
  id: string
  email: string
  name: string
  github_id?: number
  github_login?: string
  created_at: string
  updated_at: string
}

export interface AuthResponse {
  token: string
  user: User
}

export interface Project {
  id: string
  owner_id: string
  name: string
  slug: string
  github_repo?: string
  github_installation_id?: number
  framework?: string
  build_command?: string
  output_dir?: string
  root_dir?: string
  production_branch: string
  created_at: string
  updated_at: string
}

export interface CreateProjectRequest {
  name: string
  github_repo?: string
  framework?: string
  build_command?: string
  output_dir?: string
  production_branch?: string
}

export interface UpdateProjectRequest {
  name?: string
  build_command?: string
  output_dir?: string
  root_dir?: string
  production_branch?: string
}

export type EnvVarTarget = "build" | "runtime" | "all"

export interface EnvVarEntry {
  key: string
  value: string
  target: EnvVarTarget
}

export type DeploymentState = "queued" | "building" | "uploading" | "ready" | "error" | "cancelled"

export interface Deployment {
  id: string
  project_id: string
  commit_sha: string
  commit_message?: string
  branch: string
  state: DeploymentState
  url?: string
  is_production: boolean
  build_log?: string
  build_started_at?: string
  build_finished_at?: string
  created_at: string
  updated_at: string
}

export interface CreateDeploymentRequest {
  commit_sha: string
  commit_message?: string
  branch: string
}

export interface ApiKey {
  id: string
  user_id: string
  name: string
  key_plain?: string
  last_used_at?: string
  expires_at?: string
  created_at: string
}

export interface GitHubRepo {
  id: number
  name: string
  full_name: string
  description?: string
  private: boolean
  default_branch: string
  html_url: string
}
