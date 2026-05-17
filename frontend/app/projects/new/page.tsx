"use client"

import { useState } from "react"
import { useRouter } from "next/navigation"
import Link from "next/link"
import { useAuth } from "@/lib/auth-context"
import { api } from "@/lib/api"
import { DashboardLayout } from "@/components/dashboard-layout"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { ArrowLeft, FolderGit2 } from "lucide-react"
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
  const { isAuthenticated, isLoading: authLoading } = useAuth()
  const router = useRouter()
  
  const [name, setName] = useState("")
  const [githubRepo, setGithubRepo] = useState("")
  const [framework, setFramework] = useState("")
  const [buildCommand, setBuildCommand] = useState("")
  const [outputDir, setOutputDir] = useState("")
  const [productionBranch, setProductionBranch] = useState("main")
  const [error, setError] = useState("")
  const [isLoading, setIsLoading] = useState(false)

  if (authLoading || !isAuthenticated) {
    return (
      <div className="flex min-h-screen items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-foreground border-t-transparent" />
      </div>
    )
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError("")
    setIsLoading(true)

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

  // Set sensible defaults based on framework
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

  return (
    <DashboardLayout>
      <div className="mx-auto max-w-2xl px-4 py-8 sm:px-6 lg:px-8">
        {/* Back link */}
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
                <Label htmlFor="github">GitHub Repository</Label>
                <Input
                  id="github"
                  placeholder="owner/repo"
                  value={githubRepo}
                  onChange={(e) => setGithubRepo(e.target.value)}
                />
                <p className="text-xs text-muted-foreground">
                  Optional. Link a GitHub repository to enable automatic deployments.
                </p>
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
