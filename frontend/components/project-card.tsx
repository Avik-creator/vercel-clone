"use client"

import Link from "next/link"
import { Project, Deployment } from "@/lib/api"
import { Card, CardContent } from "@/components/ui/card"
import { DeploymentStatusBadge } from "@/components/deployment-status-badge"
import { formatRelativeTime, truncateCommitSha } from "@/lib/utils"
import { GitBranch, GitCommit, ExternalLink, MoreHorizontal } from "lucide-react"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"

interface ProjectCardProps {
  project: Project
  latestDeployment?: Deployment
}

export function ProjectCard({ project, latestDeployment }: ProjectCardProps) {
  return (
    <Card className="group relative overflow-hidden transition-colors hover:bg-card/80">
      <CardContent className="p-5">
        <div className="flex items-start justify-between gap-4">
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <Link 
                href={`/projects/${project.id}`}
                className="font-semibold text-foreground hover:underline truncate"
              >
                {project.name}
              </Link>
              {latestDeployment && (
                <DeploymentStatusBadge state={latestDeployment.state} />
              )}
            </div>
            
            {project.github_repo && (
              <p className="mt-1 text-sm text-muted-foreground truncate">
                {project.github_repo}
              </p>
            )}

            {latestDeployment && (
              <div className="mt-3 flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-muted-foreground">
                <span className="flex items-center gap-1">
                  <GitBranch className="h-3 w-3" />
                  {latestDeployment.branch}
                </span>
                <span className="flex items-center gap-1">
                  <GitCommit className="h-3 w-3" />
                  {truncateCommitSha(latestDeployment.commit_sha)}
                </span>
                <span>{formatRelativeTime(latestDeployment.created_at)}</span>
              </div>
            )}

            {latestDeployment?.url && latestDeployment.state === "ready" && (
              <a
                href={`https://${latestDeployment.url}`}
                target="_blank"
                rel="noopener noreferrer"
                className="mt-2 inline-flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
              >
                <ExternalLink className="h-3 w-3" />
                {latestDeployment.url}
              </a>
            )}
          </div>

          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="icon" className="h-8 w-8 shrink-0">
                <MoreHorizontal className="h-4 w-4" />
                <span className="sr-only">Open menu</span>
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem asChild>
                <Link href={`/projects/${project.id}`}>View Project</Link>
              </DropdownMenuItem>
              <DropdownMenuItem asChild>
                <Link href={`/projects/${project.id}/deployments`}>Deployments</Link>
              </DropdownMenuItem>
              <DropdownMenuItem asChild>
                <Link href={`/projects/${project.id}/settings`}>Settings</Link>
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </CardContent>
    </Card>
  )
}
