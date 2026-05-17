"use client"

import { useEffect, useState } from "react"
import { useRouter } from "next/navigation"
import Link from "next/link"
import { useAuth } from "@/lib/auth-context"
import { useDeployments, useProjects } from "@/lib/hooks"
import { DashboardLayout } from "@/components/dashboard-layout"
import { DeploymentRow } from "@/components/deployment-row"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Skeleton } from "@/components/ui/skeleton"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Search, Rocket, Filter } from "lucide-react"
import { DeploymentState } from "@/lib/api"

export default function DeploymentsPage() {
  const { isAuthenticated, isLoading: authLoading } = useAuth()
  const router = useRouter()
  const { data: deployments, isLoading: deploymentsLoading } = useDeployments()
  const { data: projects } = useProjects()
  const [search, setSearch] = useState("")
  const [statusFilter, setStatusFilter] = useState<string>("all")
  const [projectFilter, setProjectFilter] = useState<string>("all")

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

  const filteredDeployments = deployments?.filter(deployment => {
    const project = projects?.find(p => p.id === deployment.project_id)
    const matchesSearch = 
      deployment.commit_sha.toLowerCase().includes(search.toLowerCase()) ||
      deployment.commit_message?.toLowerCase().includes(search.toLowerCase()) ||
      deployment.branch.toLowerCase().includes(search.toLowerCase()) ||
      project?.name.toLowerCase().includes(search.toLowerCase())
    
    const matchesStatus = statusFilter === "all" || deployment.state === statusFilter
    const matchesProject = projectFilter === "all" || deployment.project_id === projectFilter

    return matchesSearch && matchesStatus && matchesProject
  }) || []

  return (
    <DashboardLayout>
      <div className="mx-auto max-w-7xl px-4 py-8 sm:px-6 lg:px-8">
        {/* Header */}
        <div className="mb-8">
          <h1 className="text-2xl font-bold text-foreground">Deployments</h1>
          <p className="text-muted-foreground mt-1">
            View and manage all your deployments across projects
          </p>
        </div>

        {/* Filters */}
        <div className="flex flex-col sm:flex-row gap-4 mb-6">
          <div className="relative flex-1 max-w-sm">
            <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              placeholder="Search deployments..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="pl-10"
            />
          </div>
          <div className="flex gap-3">
            <Select value={statusFilter} onValueChange={setStatusFilter}>
              <SelectTrigger className="w-36">
                <Filter className="h-4 w-4 mr-2" />
                <SelectValue placeholder="Status" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All Status</SelectItem>
                <SelectItem value="queued">Queued</SelectItem>
                <SelectItem value="building">Building</SelectItem>
                <SelectItem value="uploading">Uploading</SelectItem>
                <SelectItem value="ready">Ready</SelectItem>
                <SelectItem value="error">Error</SelectItem>
                <SelectItem value="cancelled">Cancelled</SelectItem>
              </SelectContent>
            </Select>
            <Select value={projectFilter} onValueChange={setProjectFilter}>
              <SelectTrigger className="w-44">
                <SelectValue placeholder="All Projects" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All Projects</SelectItem>
                {projects?.map(project => (
                  <SelectItem key={project.id} value={project.id}>
                    {project.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>

        {/* Deployments List */}
        <Card>
          <CardHeader>
            <CardTitle>All Deployments</CardTitle>
          </CardHeader>
          {deploymentsLoading ? (
            <CardContent className="p-0">
              {[1, 2, 3, 4, 5].map((i) => (
                <div key={i} className="p-4 border-b border-border last:border-0">
                  <Skeleton className="h-5 w-24 mb-2" />
                  <Skeleton className="h-4 w-48 mb-2" />
                  <Skeleton className="h-3 w-32" />
                </div>
              ))}
            </CardContent>
          ) : filteredDeployments.length > 0 ? (
            <div>
              {filteredDeployments.map((deployment) => {
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
          ) : deployments && deployments.length > 0 ? (
            <CardContent className="text-center py-8">
              <Search className="mx-auto h-12 w-12 text-muted-foreground mb-4" />
              <h3 className="font-semibold text-foreground mb-1">No deployments found</h3>
              <p className="text-sm text-muted-foreground">
                Try adjusting your filters
              </p>
            </CardContent>
          ) : (
            <CardContent className="text-center py-8">
              <Rocket className="mx-auto h-12 w-12 text-muted-foreground mb-4" />
              <h3 className="font-semibold text-foreground mb-1">No deployments yet</h3>
              <p className="text-sm text-muted-foreground">
                Create a project and push code to trigger your first deployment.
              </p>
            </CardContent>
          )}
        </Card>
      </div>
    </DashboardLayout>
  )
}
