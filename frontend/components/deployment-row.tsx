"use client"

import Link from "next/link"
import { Deployment } from "@/lib/api"
import { DeploymentStatusBadge } from "@/components/deployment-status-badge"
import { formatRelativeTime, truncateCommitSha, truncateString, deploymentPublicUrl } from "@/lib/utils"
import { GitBranch, GitCommit, ExternalLink, MoreHorizontal, Clock } from "lucide-react"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { api } from "@/lib/api"
import { useState } from "react"
import { mutate } from "swr"

interface DeploymentRowProps {
  deployment: Deployment
  showProject?: boolean
  projectName?: string
}

export function DeploymentRow({ deployment, showProject, projectName }: DeploymentRowProps) {
  const [isLoading, setIsLoading] = useState(false)

  const handleCancel = async () => {
    if (!confirm("Are you sure you want to cancel this deployment?")) return
    setIsLoading(true)
    try {
      await api.cancelDeployment(deployment.id)
      mutate(`deployment-${deployment.id}`)
      mutate(`project-${deployment.project_id}-deployments`)
      mutate("deployments")
    } catch (error) {
      console.error("Failed to cancel deployment:", error)
    } finally {
      setIsLoading(false)
    }
  }

  const handlePromote = async () => {
    if (!confirm("Are you sure you want to promote this deployment to production?")) return
    setIsLoading(true)
    try {
      await api.promoteDeployment(deployment.id)
      mutate(`deployment-${deployment.id}`)
      mutate(`project-${deployment.project_id}-deployments`)
      mutate("deployments")
    } catch (error) {
      console.error("Failed to promote deployment:", error)
    } finally {
      setIsLoading(false)
    }
  }

  const canCancel = ["queued", "building"].includes(deployment.state)
  const canPromote = deployment.state === "ready" && !deployment.is_production

  return (
    <div className="flex items-center gap-4 py-4 px-4 hover:bg-muted/50 transition-colors border-b border-border last:border-0">
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-3">
          <Link 
            href={`/deployments/${deployment.id}`}
            className="font-medium text-foreground hover:underline"
          >
            {truncateCommitSha(deployment.commit_sha)}
          </Link>
          <DeploymentStatusBadge state={deployment.state} />
          {deployment.is_production && (
            <span className="px-2 py-0.5 text-xs font-medium bg-foreground text-background rounded">
              Production
            </span>
          )}
        </div>
        
        {deployment.commit_message && (
          <p className="mt-1 text-sm text-muted-foreground truncate">
            {truncateString(deployment.commit_message, 60)}
          </p>
        )}

        <div className="mt-2 flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-muted-foreground">
          {showProject && projectName && (
            <span className="font-medium text-foreground">{projectName}</span>
          )}
          <span className="flex items-center gap-1">
            <GitBranch className="h-3 w-3" />
            {deployment.branch}
          </span>
          <span className="flex items-center gap-1">
            <GitCommit className="h-3 w-3" />
            {truncateCommitSha(deployment.commit_sha)}
          </span>
          <span className="flex items-center gap-1">
            <Clock className="h-3 w-3" />
            {formatRelativeTime(deployment.created_at)}
          </span>
        </div>
      </div>

      <div className="flex items-center gap-2 shrink-0">
        {deployment.url && deployment.state === "ready" && (
          <Button variant="outline" size="sm" asChild>
            <a
              href={deploymentPublicUrl(deployment.url)}
              target="_blank"
              rel="noopener noreferrer"
            >
              <ExternalLink className="h-3.5 w-3.5 mr-1.5" />
              Visit
            </a>
          </Button>
        )}

        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="icon" className="h-8 w-8" disabled={isLoading}>
              <MoreHorizontal className="h-4 w-4" />
              <span className="sr-only">Open menu</span>
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem asChild>
              <Link href={`/deployments/${deployment.id}`}>View Details</Link>
            </DropdownMenuItem>
            <DropdownMenuItem asChild>
              <Link href={`/deployments/${deployment.id}#logs`}>View Logs</Link>
            </DropdownMenuItem>
            {(canCancel || canPromote) && <DropdownMenuSeparator />}
            {canPromote && (
              <DropdownMenuItem onClick={handlePromote}>
                Promote to Production
              </DropdownMenuItem>
            )}
            {canCancel && (
              <DropdownMenuItem 
                onClick={handleCancel}
                className="text-destructive focus:text-destructive"
              >
                Cancel Deployment
              </DropdownMenuItem>
            )}
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>
  )
}
