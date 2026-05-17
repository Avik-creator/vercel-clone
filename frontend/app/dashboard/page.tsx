"use client"

import { useEffect } from "react"
import { useRouter } from "next/navigation"
import { useAuth } from "@/lib/auth-context"
import { useProjects, useDeployments } from "@/lib/hooks"
import { DashboardLayout } from "@/components/dashboard-layout"
import { ProjectCard } from "@/components/project-card"
import { DeploymentRow } from "@/components/deployment-row"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Skeleton } from "@/components/ui/skeleton"
import { Plus, Rocket, FolderGit2, Activity } from "lucide-react"
import Link from "next/link"

export default function DashboardPage() {
  const { isAuthenticated, isLoading: authLoading } = useAuth()
  const router = useRouter()
  const { data: projects, isLoading: projectsLoading } = useProjects()
  const { data: deployments, isLoading: deploymentsLoading } = useDeployments()

  useEffect(() => {
    if (!authLoading && !isAuthenticated) {
      router.push("/login")
    }
  }, [authLoading, isAuthenticated, router])

  if (authLoading || !isAuthenticated) {
    return (
      <div className="flex min-h-screen items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-foreground border-t-transparent" />
      </div>
    )
  }

  const recentDeployments = deployments?.slice(0, 5) || []
  const recentProjects = projects?.slice(0, 4) || []

  // Find latest deployment for each project
  const projectDeployments = new Map<string, typeof deployments>()
  if (deployments && projects) {
    projects.forEach(project => {
      const latest = deployments.find(d => d.project_id === project.id)
      if (latest) {
        projectDeployments.set(project.id, [latest])
      }
    })
  }

  return (
    <DashboardLayout>
      <div className="mx-auto max-w-7xl px-4 py-8 sm:px-6 lg:px-8">
        {/* Header */}
        <div className="flex items-center justify-between mb-8">
          <div>
            <h1 className="text-2xl font-bold text-foreground">Overview</h1>
            <p className="text-muted-foreground mt-1">
              Welcome back! Here&apos;s what&apos;s happening with your projects.
            </p>
          </div>
          <Link href="/projects/new">
            <Button className="gap-2">
              <Plus className="h-4 w-4" />
              New Project
            </Button>
          </Link>
        </div>

        {/* Stats */}
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-3 mb-8">
          <Card>
            <CardContent className="p-6">
              <div className="flex items-center gap-4">
                <div className="flex h-12 w-12 items-center justify-center rounded-lg bg-muted">
                  <FolderGit2 className="h-6 w-6 text-foreground" />
                </div>
                <div>
                  <p className="text-sm text-muted-foreground">Total Projects</p>
                  <p className="text-2xl font-bold text-foreground">
                    {projectsLoading ? <Skeleton className="h-8 w-12" /> : projects?.length || 0}
                  </p>
                </div>
              </div>
            </CardContent>
          </Card>
          <Card>
            <CardContent className="p-6">
              <div className="flex items-center gap-4">
                <div className="flex h-12 w-12 items-center justify-center rounded-lg bg-muted">
                  <Rocket className="h-6 w-6 text-foreground" />
                </div>
                <div>
                  <p className="text-sm text-muted-foreground">Total Deployments</p>
                  <p className="text-2xl font-bold text-foreground">
                    {deploymentsLoading ? <Skeleton className="h-8 w-12" /> : deployments?.length || 0}
                  </p>
                </div>
              </div>
            </CardContent>
          </Card>
          <Card>
            <CardContent className="p-6">
              <div className="flex items-center gap-4">
                <div className="flex h-12 w-12 items-center justify-center rounded-lg bg-muted">
                  <Activity className="h-6 w-6 text-foreground" />
                </div>
                <div>
                  <p className="text-sm text-muted-foreground">Active Builds</p>
                  <p className="text-2xl font-bold text-foreground">
                    {deploymentsLoading ? (
                      <Skeleton className="h-8 w-12" />
                    ) : (
                      deployments?.filter(d => ["queued", "building"].includes(d.state)).length || 0
                    )}
                  </p>
                </div>
              </div>
            </CardContent>
          </Card>
        </div>

        <div className="grid grid-cols-1 gap-8 lg:grid-cols-2">
          {/* Recent Projects */}
          <div>
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-lg font-semibold text-foreground">Recent Projects</h2>
              <Link href="/projects" className="text-sm text-muted-foreground hover:text-foreground">
                View all
              </Link>
            </div>
            {projectsLoading ? (
              <div className="space-y-4">
                {[1, 2, 3].map((i) => (
                  <Card key={i}>
                    <CardContent className="p-5">
                      <Skeleton className="h-5 w-32 mb-2" />
                      <Skeleton className="h-4 w-48 mb-3" />
                      <Skeleton className="h-3 w-full" />
                    </CardContent>
                  </Card>
                ))}
              </div>
            ) : recentProjects.length > 0 ? (
              <div className="space-y-4">
                {recentProjects.map((project) => (
                  <ProjectCard
                    key={project.id}
                    project={project}
                    latestDeployment={projectDeployments.get(project.id)?.[0]}
                  />
                ))}
              </div>
            ) : (
              <Card>
                <CardContent className="p-8 text-center">
                  <FolderGit2 className="mx-auto h-12 w-12 text-muted-foreground mb-4" />
                  <h3 className="font-semibold text-foreground mb-1">No projects yet</h3>
                  <p className="text-sm text-muted-foreground mb-4">
                    Create your first project to get started.
                  </p>
                  <Link href="/projects/new">
                    <Button size="sm">Create Project</Button>
                  </Link>
                </CardContent>
              </Card>
            )}
          </div>

          {/* Recent Deployments */}
          <div>
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-lg font-semibold text-foreground">Recent Deployments</h2>
              <Link href="/deployments" className="text-sm text-muted-foreground hover:text-foreground">
                View all
              </Link>
            </div>
            <Card>
              {deploymentsLoading ? (
                <CardContent className="p-0">
                  {[1, 2, 3].map((i) => (
                    <div key={i} className="p-4 border-b border-border last:border-0">
                      <Skeleton className="h-5 w-24 mb-2" />
                      <Skeleton className="h-4 w-48 mb-2" />
                      <Skeleton className="h-3 w-32" />
                    </div>
                  ))}
                </CardContent>
              ) : recentDeployments.length > 0 ? (
                <div>
                  {recentDeployments.map((deployment) => {
                    const project = projects?.find(p => p.id === deployment.project_id)
                    return (
                      <DeploymentRow
                        key={deployment.id}
                        deployment={deployment}
                        showProject
                        projectName={project?.name}
                      />
                    )
                  })}
                </div>
              ) : (
                <CardContent className="p-8 text-center">
                  <Rocket className="mx-auto h-12 w-12 text-muted-foreground mb-4" />
                  <h3 className="font-semibold text-foreground mb-1">No deployments yet</h3>
                  <p className="text-sm text-muted-foreground">
                    Create a project and push code to trigger your first deployment.
                  </p>
                </CardContent>
              )}
            </Card>
          </div>
        </div>
      </div>
    </DashboardLayout>
  )
}
