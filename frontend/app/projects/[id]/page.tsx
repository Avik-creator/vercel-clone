"use client"

import { useEffect, use } from "react"
import { useRouter } from "next/navigation"
import Link from "next/link"
import { useAuth } from "@/lib/auth-context"
import { useProject, useProjectDeployments } from "@/lib/hooks"
import { DashboardLayout } from "@/components/dashboard-layout"
import { DeploymentRow } from "@/components/deployment-row"
import { DeploymentStatusBadge } from "@/components/deployment-status-badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Skeleton } from "@/components/ui/skeleton"
import { 
  ArrowLeft, 
  ExternalLink, 
  Settings, 
  GitBranch,
  Rocket,
  Clock
} from "lucide-react"
import { GitHubIcon } from "@/components/icons/github"
import { formatRelativeTime } from "@/lib/utils"

export default function ProjectDetailPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = use(params)
  const { isAuthenticated, isLoading: authLoading } = useAuth()
  const router = useRouter()
  const { data: project, isLoading: projectLoading } = useProject(id)
  const { data: deployments, isLoading: deploymentsLoading } = useProjectDeployments(id)

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

  const latestDeployment = deployments?.[0]
  const productionDeployment = deployments?.find(d => d.is_production && d.state === "ready")

  return (
    <DashboardLayout>
      <div className="mx-auto max-w-7xl px-4 py-8 sm:px-6 lg:px-8">
        {/* Back link */}
        <Link
          href="/projects"
          className="inline-flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground mb-6"
        >
          <ArrowLeft className="h-4 w-4" />
          Back to Projects
        </Link>

        {projectLoading ? (
          <div className="space-y-6">
            <div className="flex items-center justify-between">
              <Skeleton className="h-8 w-48" />
              <Skeleton className="h-9 w-24" />
            </div>
            <Skeleton className="h-32 w-full" />
          </div>
        ) : project ? (
          <>
            {/* Header */}
            <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 mb-8">
              <div>
                <div className="flex items-center gap-3">
                  <h1 className="text-2xl font-bold text-foreground">{project.name}</h1>
                  {latestDeployment && (
                    <DeploymentStatusBadge state={latestDeployment.state} />
                  )}
                </div>
                {project.github_repo && (
                  <a
                    href={`https://github.com/${project.github_repo}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="inline-flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground mt-1"
                  >
                    <GitHubIcon className="h-4 w-4" />
                    {project.github_repo}
                    <ExternalLink className="h-3 w-3" />
                  </a>
                )}
              </div>
              <div className="flex items-center gap-3">
                {productionDeployment?.url && (
                  <Button variant="outline" asChild>
                    <a
                      href={`https://${productionDeployment.url}`}
                      target="_blank"
                      rel="noopener noreferrer"
                    >
                      <ExternalLink className="h-4 w-4 mr-2" />
                      Visit
                    </a>
                  </Button>
                )}
                <Link href={`/projects/${id}/settings`}>
                  <Button variant="outline">
                    <Settings className="h-4 w-4 mr-2" />
                    Settings
                  </Button>
                </Link>
              </div>
            </div>

            {/* Project Info Cards */}
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-3 mb-8">
              <Card>
                <CardContent className="p-4">
                  <div className="flex items-center gap-3">
                    <GitBranch className="h-5 w-5 text-muted-foreground" />
                    <div>
                      <p className="text-sm text-muted-foreground">Production Branch</p>
                      <p className="font-medium text-foreground">{project.production_branch}</p>
                    </div>
                  </div>
                </CardContent>
              </Card>
              <Card>
                <CardContent className="p-4">
                  <div className="flex items-center gap-3">
                    <Rocket className="h-5 w-5 text-muted-foreground" />
                    <div>
                      <p className="text-sm text-muted-foreground">Total Deployments</p>
                      <p className="font-medium text-foreground">
                        {deploymentsLoading ? "..." : deployments?.length || 0}
                      </p>
                    </div>
                  </div>
                </CardContent>
              </Card>
              <Card>
                <CardContent className="p-4">
                  <div className="flex items-center gap-3">
                    <Clock className="h-5 w-5 text-muted-foreground" />
                    <div>
                      <p className="text-sm text-muted-foreground">Created</p>
                      <p className="font-medium text-foreground">
                        {formatRelativeTime(project.created_at)}
                      </p>
                    </div>
                  </div>
                </CardContent>
              </Card>
            </div>

            {/* Tabs */}
            <Tabs defaultValue="deployments">
              <TabsList>
                <TabsTrigger value="deployments">Deployments</TabsTrigger>
                <TabsTrigger value="settings">Build Settings</TabsTrigger>
              </TabsList>

              <TabsContent value="deployments" className="mt-6">
                <Card>
                  <CardHeader>
                    <CardTitle>Deployments</CardTitle>
                  </CardHeader>
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
                  ) : deployments && deployments.length > 0 ? (
                    <div>
                      {deployments.map((deployment) => (
                        <DeploymentRow
                          key={deployment.id}
                          deployment={deployment}
                        />
                      ))}
                    </div>
                  ) : (
                    <CardContent className="text-center py-8">
                      <Rocket className="mx-auto h-12 w-12 text-muted-foreground mb-4" />
                      <h3 className="font-semibold text-foreground mb-1">No deployments yet</h3>
                      <p className="text-sm text-muted-foreground">
                        Push code to your repository to trigger your first deployment.
                      </p>
                    </CardContent>
                  )}
                </Card>
              </TabsContent>

              <TabsContent value="settings" className="mt-6">
                <Card>
                  <CardHeader>
                    <CardTitle>Build Settings</CardTitle>
                  </CardHeader>
                  <CardContent className="space-y-4">
                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                      <div>
                        <p className="text-sm text-muted-foreground">Framework</p>
                        <p className="font-medium text-foreground">
                          {project.framework || "Not specified"}
                        </p>
                      </div>
                      <div>
                        <p className="text-sm text-muted-foreground">Root Directory</p>
                        <p className="font-medium text-foreground font-mono text-sm">
                          {project.root_dir || "./"}
                        </p>
                      </div>
                      <div>
                        <p className="text-sm text-muted-foreground">Build Command</p>
                        <p className="font-medium text-foreground font-mono text-sm">
                          {project.build_command || "npm run build"}
                        </p>
                      </div>
                      <div>
                        <p className="text-sm text-muted-foreground">Output Directory</p>
                        <p className="font-medium text-foreground font-mono text-sm">
                          {project.output_dir || "dist"}
                        </p>
                      </div>
                    </div>
                    <div className="pt-4">
                      <Link href={`/projects/${id}/settings`}>
                        <Button variant="outline" size="sm">
                          Edit Settings
                        </Button>
                      </Link>
                    </div>
                  </CardContent>
                </Card>
              </TabsContent>
            </Tabs>
          </>
        ) : (
          <div className="text-center py-12">
            <h2 className="text-xl font-semibold text-foreground mb-2">Project not found</h2>
            <p className="text-muted-foreground mb-4">
              The project you&apos;re looking for doesn&apos;t exist or you don&apos;t have access.
            </p>
            <Link href="/projects">
              <Button>Back to Projects</Button>
            </Link>
          </div>
        )}
      </div>
    </DashboardLayout>
  )
}
