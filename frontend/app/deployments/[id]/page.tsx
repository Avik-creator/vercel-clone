"use client"

import { useEffect, useState, useRef, use } from "react"
import { useRouter } from "next/navigation"
import Link from "next/link"
import { useAuth } from "@/lib/auth-context"
import { useDeployment, useProject } from "@/lib/hooks"
import { api } from "@/lib/api"
import { DashboardLayout } from "@/components/dashboard-layout"
import { DeploymentStatusBadge } from "@/components/deployment-status-badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Skeleton } from "@/components/ui/skeleton"
import { 
  ArrowLeft, 
  ExternalLink, 
  GitBranch, 
  GitCommit, 
  Clock,
  Terminal,
  Copy,
  Check,
  RotateCw,
  XCircle
} from "lucide-react"
import { formatDate, formatRelativeTime, truncateCommitSha } from "@/lib/utils"
import { mutate } from "swr"

export default function DeploymentDetailPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = use(params)
  const { isAuthenticated, isLoading: authLoading } = useAuth()
  const router = useRouter()
  const { data: deployment, isLoading: deploymentLoading } = useDeployment(id)
  const { data: project } = useProject(deployment?.project_id)
  
  const [logs, setLogs] = useState<string[]>([])
  const [isStreaming, setIsStreaming] = useState(false)
  const [copied, setCopied] = useState(false)
  const logsEndRef = useRef<HTMLDivElement>(null)
  const cleanupRef = useRef<(() => void) | null>(null)

  useEffect(() => {
    if (!authLoading && !isAuthenticated) {
      router.push("/login")
    }
  }, [authLoading, isAuthenticated, router])

  // Stream logs for active deployments
  useEffect(() => {
    if (!deployment || !["queued", "building", "uploading"].includes(deployment.state)) {
      return
    }

    setIsStreaming(true)
    cleanupRef.current = api.streamDeploymentLogs(
      id,
      (log) => {
        setLogs(prev => [...prev, log])
      },
      () => {
        setIsStreaming(false)
      }
    )

    return () => {
      cleanupRef.current?.()
    }
  }, [deployment?.state, id])

  // Auto-scroll logs
  useEffect(() => {
    logsEndRef.current?.scrollIntoView({ behavior: "smooth" })
  }, [logs])

  // Load existing build log
  useEffect(() => {
    if (deployment?.build_log && logs.length === 0) {
      setLogs(deployment.build_log.split("\n").filter(Boolean))
    }
  }, [deployment?.build_log])

  if (authLoading || !isAuthenticated) {
    return (
      <div className="flex min-h-screen items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-foreground border-t-transparent" />
      </div>
    )
  }

  const handleCancel = async () => {
    if (!confirm("Are you sure you want to cancel this deployment?")) return
    try {
      await api.cancelDeployment(id)
      mutate(`deployment-${id}`)
    } catch (error) {
      console.error("Failed to cancel:", error)
    }
  }

  const handlePromote = async () => {
    if (!confirm("Are you sure you want to promote this deployment to production?")) return
    try {
      await api.promoteDeployment(id)
      mutate(`deployment-${id}`)
    } catch (error) {
      console.error("Failed to promote:", error)
    }
  }

  const copyLogs = () => {
    navigator.clipboard.writeText(logs.join("\n"))
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  const canCancel = deployment && ["queued", "building"].includes(deployment.state)
  const canPromote = deployment?.state === "ready" && !deployment.is_production

  return (
    <DashboardLayout>
      <div className="mx-auto max-w-7xl px-4 py-8 sm:px-6 lg:px-8">
        {/* Back link */}
        <Link
          href="/deployments"
          className="inline-flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground mb-6"
        >
          <ArrowLeft className="h-4 w-4" />
          Back to Deployments
        </Link>

        {deploymentLoading ? (
          <div className="space-y-6">
            <div className="flex items-center justify-between">
              <Skeleton className="h-8 w-48" />
              <Skeleton className="h-9 w-24" />
            </div>
            <Skeleton className="h-64 w-full" />
          </div>
        ) : deployment ? (
          <>
            {/* Header */}
            <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 mb-8">
              <div>
                <div className="flex items-center gap-3">
                  <h1 className="text-2xl font-bold text-foreground font-mono">
                    {truncateCommitSha(deployment.commit_sha)}
                  </h1>
                  <DeploymentStatusBadge state={deployment.state} />
                  {deployment.is_production && (
                    <span className="px-2 py-0.5 text-xs font-medium bg-foreground text-background rounded">
                      Production
                    </span>
                  )}
                </div>
                {project && (
                  <Link
                    href={`/projects/${project.id}`}
                    className="text-sm text-muted-foreground hover:text-foreground mt-1 inline-block"
                  >
                    {project.name}
                  </Link>
                )}
              </div>
              <div className="flex items-center gap-3">
                {deployment.url && deployment.state === "ready" && (
                  <Button variant="outline" asChild>
                    <a
                      href={`https://${deployment.url}`}
                      target="_blank"
                      rel="noopener noreferrer"
                    >
                      <ExternalLink className="h-4 w-4 mr-2" />
                      Visit
                    </a>
                  </Button>
                )}
                {canPromote && (
                  <Button variant="outline" onClick={handlePromote}>
                    <RotateCw className="h-4 w-4 mr-2" />
                    Promote
                  </Button>
                )}
                {canCancel && (
                  <Button variant="destructive" onClick={handleCancel}>
                    <XCircle className="h-4 w-4 mr-2" />
                    Cancel
                  </Button>
                )}
              </div>
            </div>

            {/* Deployment Info */}
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4 mb-8">
              <Card>
                <CardContent className="p-4">
                  <div className="flex items-center gap-3">
                    <GitBranch className="h-5 w-5 text-muted-foreground" />
                    <div>
                      <p className="text-sm text-muted-foreground">Branch</p>
                      <p className="font-medium text-foreground">{deployment.branch}</p>
                    </div>
                  </div>
                </CardContent>
              </Card>
              <Card>
                <CardContent className="p-4">
                  <div className="flex items-center gap-3">
                    <GitCommit className="h-5 w-5 text-muted-foreground" />
                    <div>
                      <p className="text-sm text-muted-foreground">Commit</p>
                      <p className="font-medium text-foreground font-mono text-sm">
                        {truncateCommitSha(deployment.commit_sha)}
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
                        {formatRelativeTime(deployment.created_at)}
                      </p>
                    </div>
                  </div>
                </CardContent>
              </Card>
              <Card>
                <CardContent className="p-4">
                  <div className="flex items-center gap-3">
                    <Terminal className="h-5 w-5 text-muted-foreground" />
                    <div>
                      <p className="text-sm text-muted-foreground">Build Time</p>
                      <p className="font-medium text-foreground">
                        {deployment.build_started_at && deployment.build_finished_at
                          ? `${Math.round((new Date(deployment.build_finished_at).getTime() - new Date(deployment.build_started_at).getTime()) / 1000)}s`
                          : deployment.build_started_at
                          ? "In progress..."
                          : "Pending"}
                      </p>
                    </div>
                  </div>
                </CardContent>
              </Card>
            </div>

            {/* Commit Message */}
            {deployment.commit_message && (
              <Card className="mb-8">
                <CardContent className="p-4">
                  <p className="text-sm text-muted-foreground mb-1">Commit Message</p>
                  <p className="text-foreground">{deployment.commit_message}</p>
                </CardContent>
              </Card>
            )}

            {/* Build Logs */}
            <Card id="logs">
              <CardHeader className="flex flex-row items-center justify-between">
                <CardTitle className="flex items-center gap-2">
                  <Terminal className="h-5 w-5" />
                  Build Logs
                  {isStreaming && (
                    <span className="ml-2 h-2 w-2 rounded-full bg-success animate-pulse" />
                  )}
                </CardTitle>
                <Button variant="outline" size="sm" onClick={copyLogs} disabled={logs.length === 0}>
                  {copied ? (
                    <>
                      <Check className="h-4 w-4 mr-1" />
                      Copied
                    </>
                  ) : (
                    <>
                      <Copy className="h-4 w-4 mr-1" />
                      Copy
                    </>
                  )}
                </Button>
              </CardHeader>
              <CardContent className="p-0">
                <div className="bg-black rounded-b-lg max-h-[500px] overflow-auto">
                  {logs.length > 0 ? (
                    <div className="p-4 font-mono text-sm">
                      {logs.map((log, i) => (
                        <div key={i} className="log-line text-foreground/90">
                          {log}
                        </div>
                      ))}
                      <div ref={logsEndRef} />
                    </div>
                  ) : (
                    <div className="p-8 text-center text-muted-foreground">
                      {["queued"].includes(deployment.state)
                        ? "Waiting for build to start..."
                        : "No build logs available"}
                    </div>
                  )}
                </div>
              </CardContent>
            </Card>

            {/* URL Info */}
            {deployment.url && (
              <Card className="mt-8">
                <CardHeader>
                  <CardTitle>Deployment URL</CardTitle>
                </CardHeader>
                <CardContent>
                  <a
                    href={`https://${deployment.url}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="inline-flex items-center gap-2 text-foreground hover:underline font-mono text-sm"
                  >
                    https://{deployment.url}
                    <ExternalLink className="h-4 w-4" />
                  </a>
                </CardContent>
              </Card>
            )}
          </>
        ) : (
          <div className="text-center py-12">
            <h2 className="text-xl font-semibold text-foreground mb-2">Deployment not found</h2>
            <p className="text-muted-foreground mb-4">
              The deployment you&apos;re looking for doesn&apos;t exist or you don&apos;t have access.
            </p>
            <Link href="/deployments">
              <Button>Back to Deployments</Button>
            </Link>
          </div>
        )}
      </div>
    </DashboardLayout>
  )
}
