import useSWR from "swr"
import { api, Project, Deployment, ApiKey, EnvVarEntry } from "@/lib/api"

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

export function useDeployment(id: string | undefined) {
  return useSWR<Deployment>(
    id ? `deployment-${id}` : null,
    () => api.getDeployment(id!),
    { refreshInterval: 5000 } // Poll for status updates
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
