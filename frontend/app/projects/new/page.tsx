"use client"

import { useState, useEffect } from "react"
import { useRouter } from "next/navigation"
import Link from "next/link"
import { useAuth } from "@/lib/auth-context"
import { useGitHubRepos } from "@/lib/hooks"
import { api, GitHubRepo } from "@/lib/api"
import { DashboardLayout } from "@/components/dashboard-layout"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { ArrowLeft, FolderGit2, Lock, Search, RefreshCw, AlertCircle } from "lucide-react"
import { GitHubIcon } from "@/components/icons/github"
import { mutate } from "swr"

const FRAMEWORKS = [
  { value: "nextjs", label: "Next.js" },
  { value: "react", label: "React (Vite)" },
  { value: "vue", label: "Vue.js" },
  { value: "svelte", label: "Svelte" },
  { value: "static", label: "Static Site" },
  { value: "other", label: "Other" },
]

export default function NewProjectPage() {
  const { user, isAuthenticated, isLoading: authLoading } = useAuth()
  const router = useRouter()
  const { data: repos, error: reposError, isLoading: reposLoading, mutate: mutateRepos } = useGitHubRepos()
  
  const [name, setName] = useState("")
  const [selectedRepo, setSelectedRepo] = useState<string>("")
  const [manualRepo, setManualRepo] = useState("")
  const [useManualInput, setUseManualInput] = useState(false)
  const [framework, setFramework] = useState("")
  const [buildCommand, setBuildCommand] = useState("")
  const [outputDir, setOutputDir] = useState("")
  const [productionBranch, setProductionBranch] = useState("main")
  const [error, setError] = useState("")
  const [isLoading, setIsLoading] = useState(false)
  const [searchQuery, setSearchQuery] = useState("")

  // Auto-set project name from repo selection
  useEffect(() => {
    if (selectedRepo && !useManualInput) {
      const repo = repos?.find(r => r.full_name === selectedRepo)
      if (repo && !name) {
        setName(repo.name)
        setProductionBranch(repo.default_branch)
      }
    }
  }, [selectedRepo, repos, name, useManualInput])

  if (authLoading || !isAuthenticated) {
    return (
      <div className="flex min-h-screen items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-foreground border-t-transparent" />
      </div>
    )
  }

  const hasGitHubLinked = !!user?.github_id

  const filteredRepos = repos?.filter(repo => 
    repo.full_name.toLowerCase().includes(searchQuery.toLowerCase()) ||
    repo.description?.toLowerCase().includes(searchQuery.toLowerCase())
  ) || []

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError("")
    setIsLoading(true)

    const githubRepo = useManualInput ? manualRepo : selectedRepo

    try {
      const project = await api.createProject({
        name,
        github_repo: githubRepo || undefined,
        framework: framework || undefined,
        build_command: buildCommand || undefined,
        output_dir: outputDir || undefined,
        production_branch: productionBranch || undefined,
      })
      mutate("projects")
      router.push(`/projects/${project.id}`)
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create project")
    } finally {
      setIsLoading(false)
    }
  }

  const handleFrameworkChange = (value: string) => {
    setFramework(value)
    switch (value) {
      case "nextjs":
        setBuildCommand("npm run build")
        setOutputDir(".next")
        break
      case "react":
        setBuildCommand("npm run build")
        setOutputDir("dist")
        break
      case "vue":
        setBuildCommand("npm run build")
        setOutputDir("dist")
        break
      case "svelte":
        setBuildCommand("npm run build")
        setOutputDir("build")
        break
      case "static":
        setBuildCommand("")
        setOutputDir("public")
        break
      default:
        setBuildCommand("npm run build")
        setOutputDir("dist")
    }
  }

  const handleRepoSelect = (fullName: string) => {
    setSelectedRepo(fullName)
    const repo = repos?.find(r => r.full_name === fullName)
    if (repo) {
      if (!name) setName(repo.name)
      setProductionBranch(repo.default_branch)
    }
  }

  return (
    <DashboardLayout>
      <div className="mx-auto max-w-2xl px-4 py-8 sm:px-6 lg:px-8">
        <Link
          href="/projects"
          className="inline-flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground mb-6"
        >
          <ArrowLeft className="h-4 w-4" />
          Back to Projects
        </Link>

        <Card>
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-muted">
                <FolderGit2 className="h-5 w-5 text-foreground" />
              </div>
              <div>
                <CardTitle>Create New Project</CardTitle>
                <CardDescription>
                  Set up a new project to start deploying
                </CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent>
            <form onSubmit={handleSubmit} className="space-y-6">
              {error && (
                <div className="p-3 text-sm text-destructive bg-destructive/10 rounded-md">
                  {error}
                </div>
              )}

              {/* GitHub Repository Selection */}
              <div className="space-y-3">
                <div className="flex items-center justify-between">
                  <Label>GitHub Repository</Label>
                  {!useManualInput && hasGitHubLinked && (
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      onClick={() => mutateRepos()}
                      disabled={reposLoading}
                      className="h-8 text-xs"
                    >
                      <RefreshCw className={`h-3 w-3 mr-1 ${reposLoading ? "animate-spin" : ""}`} />
                      Refresh
                    </Button>
                  )}
                </div>

                {!hasGitHubLinked ? (
                  <div className="rounded-lg border border-dashed border-border p-4">
                    <div className="flex flex-col items-center gap-3 text-center">
                      <div className="rounded-full bg-muted p-2">
                        <GitHubIcon className="h-5 w-5" />
                      </div>
                      <div>
                        <p className="text-sm font-medium">Connect GitHub to import repositories</p>
                        <p className="text-xs text-muted-foreground mt-1">
                          Sign in with GitHub to access your repositories
                        </p>
                      </div>
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        onClick={() => window.location.href = api.getGitHubOAuthUrl()}
                      >
                        <GitHubIcon className="h-4 w-4 mr-2" />
                        Connect GitHub
                      </Button>
                    </div>
                  </div>
                ) : (
                  <>
                    {/* Toggle between dropdown and manual input */}
                    <div className="flex gap-2">
                      <Button
                        type="button"
                        variant={!useManualInput ? "default" : "outline"}
                        size="sm"
                        onClick={() => setUseManualInput(false)}
                      >
                        Select from GitHub
                      </Button>
                      <Button
                        type="button"
                        variant={useManualInput ? "default" : "outline"}
                        size="sm"
                        onClick={() => setUseManualInput(true)}
                      >
                        Enter manually
                      </Button>
                    </div>

                    {useManualInput ? (
                      <div className="space-y-2">
                        <Input
                          placeholder="owner/repo"
                          value={manualRepo}
                          onChange={(e) => setManualRepo(e.target.value)}
                        />
                        <p className="text-xs text-muted-foreground">
                          Enter the full repository path (e.g., octocat/hello-world)
                        </p>
                      </div>
                    ) : (
                      <div className="space-y-2">
                        {reposError ? (
                          <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-3">
                            <div className="flex items-center gap-2 text-sm text-destructive">
                              <AlertCircle className="h-4 w-4" />
                              <span>Failed to load repositories. Please try again.</span>
                            </div>
                          </div>
                        ) : (
                          <>
                            {/* Search input */}
                            <div className="relative">
                              <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
                              <Input
                                placeholder="Search repositories..."
                                value={searchQuery}
                                onChange={(e) => setSearchQuery(e.target.value)}
                                className="pl-9"
                              />
                            </div>

                            {/* Repos dropdown */}
                            <Select value={selectedRepo} onValueChange={handleRepoSelect}>
                              <SelectTrigger>
                                <SelectValue placeholder={reposLoading ? "Loading repositories..." : "Select a repository"} />
                              </SelectTrigger>
                              <SelectContent className="max-h-[300px]">
                                {reposLoading ? (
                                  <div className="p-4 text-center text-sm text-muted-foreground">
                                    Loading repositories...
                                  </div>
                                ) : filteredRepos.length === 0 ? (
                                  <div className="p-4 text-center text-sm text-muted-foreground">
                                    {searchQuery ? "No repositories found" : "No repositories available"}
                                  </div>
                                ) : (
                                  filteredRepos.map((repo) => (
                                    <SelectItem key={repo.id} value={repo.full_name}>
                                      <div className="flex items-center gap-2">
                                        {repo.private && <Lock className="h-3 w-3 text-muted-foreground" />}
                                        <span>{repo.full_name}</span>
                                      </div>
                                    </SelectItem>
                                  ))
                                )}
                              </SelectContent>
                            </Select>

                            {selectedRepo && (
                              <RepoPreview repo={repos?.find(r => r.full_name === selectedRepo)} />
                            )}
                          </>
                        )}
                      </div>
                    )}
                  </>
                )}

                <p className="text-xs text-muted-foreground">
                  Optional. Link a GitHub repository to enable automatic deployments.
                </p>
              </div>

              <div className="space-y-2">
                <Label htmlFor="name">Project Name *</Label>
                <Input
                  id="name"
                  placeholder="my-awesome-app"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  required
                />
              </div>

              <div className="space-y-2">
                <Label htmlFor="framework">Framework</Label>
                <Select value={framework} onValueChange={handleFrameworkChange}>
                  <SelectTrigger>
                    <SelectValue placeholder="Select a framework" />
                  </SelectTrigger>
                  <SelectContent>
                    {FRAMEWORKS.map((fw) => (
                      <SelectItem key={fw.value} value={fw.value}>
                        {fw.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                <div className="space-y-2">
                  <Label htmlFor="build">Build Command</Label>
                  <Input
                    id="build"
                    placeholder="npm run build"
                    value={buildCommand}
                    onChange={(e) => setBuildCommand(e.target.value)}
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="output">Output Directory</Label>
                  <Input
                    id="output"
                    placeholder="dist"
                    value={outputDir}
                    onChange={(e) => setOutputDir(e.target.value)}
                  />
                </div>
              </div>

              <div className="space-y-2">
                <Label htmlFor="branch">Production Branch</Label>
                <Input
                  id="branch"
                  placeholder="main"
                  value={productionBranch}
                  onChange={(e) => setProductionBranch(e.target.value)}
                />
                <p className="text-xs text-muted-foreground">
                  Deployments from this branch will be marked as production.
                </p>
              </div>

              <div className="flex justify-end gap-3 pt-4">
                <Link href="/projects">
                  <Button type="button" variant="outline">
                    Cancel
                  </Button>
                </Link>
                <Button type="submit" disabled={isLoading || !name}>
                  {isLoading ? "Creating..." : "Create Project"}
                </Button>
              </div>
            </form>
          </CardContent>
        </Card>
      </div>
    </DashboardLayout>
  )
}

function RepoPreview({ repo }: { repo?: GitHubRepo }) {
  if (!repo) return null

  return (
    <div className="rounded-lg border bg-muted/50 p-3">
      <div className="flex items-start justify-between">
        <div>
          <div className="flex items-center gap-2">
            <span className="font-medium text-sm">{repo.name}</span>
            {repo.private && (
              <span className="inline-flex items-center rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">
                <Lock className="h-3 w-3 mr-1" />
                Private
              </span>
            )}
          </div>
          {repo.description && (
            <p className="text-xs text-muted-foreground mt-1 line-clamp-2">{repo.description}</p>
          )}
        </div>
        <a 
          href={repo.html_url} 
          target="_blank" 
          rel="noopener noreferrer"
          className="text-xs text-muted-foreground hover:text-foreground"
        >
          View on GitHub
        </a>
      </div>
      <div className="mt-2 text-xs text-muted-foreground">
        Default branch: <span className="text-foreground">{repo.default_branch}</span>
      </div>
    </div>
  )
}
