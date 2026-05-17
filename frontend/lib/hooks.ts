import useSWR from "swr"
import { api, Project, Deployment, ApiKey, EnvVarEntry, GitHubRepo } from "@/lib/api"

// Projects
export function useProjects() {
  return useSWR<Project[]>("projects", () => api.getProjects())
}

export function useProject(id: string | undefined) {
  return useSWR<Project>(id ? `project-${id}` : null, () => api.getProject(id!))
}

// Deployments
export function useDeployments() {
  return useSWR<Deployment[]>("deployments", () => api.getDeployments())
}

export function useProjectDeployments(projectId: string | undefined) {
  return useSWR<Deployment[]>(
    projectId ? `project-${projectId}-deployments` : null,
    () => api.getProjectDeployments(projectId!)
  )
}

const ACTIVE_STATES = new Set(["queued", "building", "uploading"])

export function useDeployment(id: string | undefined) {
  return useSWR<Deployment>(
    id ? `deployment-${id}` : null,
    () => api.getDeployment(id!),
    {
      // Poll while active, stop once terminal — avoids continuous re-renders after build finishes.
      refreshInterval: (data) => (data && !ACTIVE_STATES.has(data.state) ? 0 : 3000),
      // Keep previous data during revalidation so the UI never flickers to skeleton.
      keepPreviousData: true,
    }
  )
}

// Environment Variables
export function useEnvVars(projectId: string | undefined) {
  return useSWR<EnvVarEntry[]>(
    projectId ? `project-${projectId}-env` : null,
    () => api.getEnvVars(projectId!)
  )
}

// API Keys
export function useApiKeys() {
  return useSWR<ApiKey[]>("api-keys", () => api.getApiKeys())
}

// GitHub
export function useGitHubRepos() {
  return useSWR<GitHubRepo[]>("github-repos", () => api.getGitHubRepos(), {
    revalidateOnFocus: false,
  })
}
