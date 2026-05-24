"use client"

import { useState, useEffect, use } from "react"
import { useRouter } from "next/navigation"
import Link from "next/link"
import { useAuth } from "@/lib/auth-context"
import { useProject, useEnvVars } from "@/lib/hooks"
import { api, EnvVarEntry, EnvVarTarget } from "@/lib/api"
import { DashboardLayout } from "@/components/dashboard-layout"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog"
import { ArrowLeft, Plus, Trash2, Eye, EyeOff, AlertTriangle, Upload, FileText } from "lucide-react"
import { mutate } from "swr"

export default function ProjectSettingsPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = use(params)
  const { isAuthenticated, isLoading: authLoading } = useAuth()
  const router = useRouter()
  const { data: project, isLoading: projectLoading } = useProject(id)
  const { data: envVars, isLoading: envLoading } = useEnvVars(id)

  // Project settings state
  const [name, setName] = useState("")
  const [buildCommand, setBuildCommand] = useState("")
  const [outputDir, setOutputDir] = useState("")
  const [rootDir, setRootDir] = useState("")
  const [productionBranch, setProductionBranch] = useState("")
  const [isSaving, setIsSaving] = useState(false)
  const [saveError, setSaveError] = useState("")

  // Env vars state
  const [newEnvKey, setNewEnvKey] = useState("")
  const [newEnvValue, setNewEnvValue] = useState("")
  const [newEnvTarget, setNewEnvTarget] = useState<EnvVarTarget>("all")
  const [showValues, setShowValues] = useState<Record<string, boolean>>({})
  const [isAddingEnv, setIsAddingEnv] = useState(false)

  // .env import
  const [dotenvContent, setDotenvContent] = useState("")
  const [importTarget, setImportTarget] = useState<EnvVarTarget>("all")
  const [importMerge, setImportMerge] = useState(true)
  const [isImportingEnv, setIsImportingEnv] = useState(false)
  const [importError, setImportError] = useState("")

  // Delete dialog
  const [showDeleteDialog, setShowDeleteDialog] = useState(false)
  const [isDeleting, setIsDeleting] = useState(false)

  useEffect(() => {
    if (project) {
      setName(project.name)
      setBuildCommand(project.build_command || "")
      setOutputDir(project.output_dir || "")
      setRootDir(project.root_dir || "")
      setProductionBranch(project.production_branch)
    }
  }, [project])

  useEffect(() => {
    if (!authLoading && !isAuthenticated) {
      router.push("/login")
    }
  }, [authLoading, isAuthenticated, router])

  if (authLoading || !isAuthenticated || projectLoading) {
    return (
      <div className="flex min-h-screen items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-foreground border-t-transparent" />
      </div>
    )
  }

  const handleSaveSettings = async (e: React.FormEvent) => {
    e.preventDefault()
    setSaveError("")
    setIsSaving(true)

    try {
      await api.updateProject(id, {
        name,
        build_command: buildCommand,
        output_dir: outputDir,
        root_dir: rootDir || undefined,
        production_branch: productionBranch,
      })
      mutate(`project-${id}`)
      mutate("projects")
    } catch (err) {
      setSaveError(err instanceof Error ? err.message : "Failed to save")
    } finally {
      setIsSaving(false)
    }
  }

  const handleAddEnvVar = async () => {
    if (!newEnvKey || !newEnvValue) return
    setIsAddingEnv(true)
    try {
      await api.addEnvVar(id, {
        key: newEnvKey,
        value: newEnvValue,
        target: newEnvTarget,
      })
      mutate(`project-${id}-env`)
      setNewEnvKey("")
      setNewEnvValue("")
      setNewEnvTarget("all")
    } catch (err) {
      console.error("Failed to add env var:", err)
    } finally {
      setIsAddingEnv(false)
    }
  }

  const handleDeleteEnvVar = async (key: string) => {
    try {
      await api.deleteEnvVar(id, key)
      mutate(`project-${id}-env`)
    } catch (err) {
      console.error("Failed to delete env var:", err)
    }
  }

  const handleImportEnv = async () => {
    if (!dotenvContent.trim()) return
    setImportError("")
    setIsImportingEnv(true)
    try {
      await api.importEnvVars(id, {
        content: dotenvContent,
        target: importTarget,
        merge: importMerge,
      })
      mutate(`project-${id}-env`)
      setDotenvContent("")
    } catch (err) {
      setImportError(err instanceof Error ? err.message : "Failed to import .env")
    } finally {
      setIsImportingEnv(false)
    }
  }

  const handleEnvFileUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return
    const text = await file.text()
    setDotenvContent(text)
    e.target.value = ""
  }

  const handleDeleteProject = async () => {
    setIsDeleting(true)
    try {
      await api.deleteProject(id)
      mutate("projects")
      router.push("/projects")
    } catch (err) {
      console.error("Failed to delete project:", err)
    } finally {
      setIsDeleting(false)
    }
  }

  const toggleShowValue = (key: string) => {
    setShowValues(prev => ({ ...prev, [key]: !prev[key] }))
  }

  if (!project) {
    return (
      <DashboardLayout>
        <div className="text-center py-12">
          <h2 className="text-xl font-semibold mb-2">Project not found</h2>
          <Link href="/projects">
            <Button>Back to Projects</Button>
          </Link>
        </div>
      </DashboardLayout>
    )
  }

  return (
    <DashboardLayout>
      <div className="mx-auto max-w-4xl px-4 py-8 sm:px-6 lg:px-8">
        <Link
          href={`/projects/${id}`}
          className="inline-flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground mb-6"
        >
          <ArrowLeft className="h-4 w-4" />
          Back to {project.name}
        </Link>

        <h1 className="text-2xl font-bold text-foreground mb-8">Project Settings</h1>

        <div className="space-y-8">
          {/* General Settings */}
          <Card>
            <CardHeader>
              <CardTitle>General</CardTitle>
              <CardDescription>Basic project configuration</CardDescription>
            </CardHeader>
            <CardContent>
              <form onSubmit={handleSaveSettings} className="space-y-4">
                {saveError && (
                  <div className="p-3 text-sm text-destructive bg-destructive/10 rounded-md">
                    {saveError}
                  </div>
                )}

                <div className="space-y-2">
                  <Label htmlFor="name">Project Name</Label>
                  <Input
                    id="name"
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                  />
                </div>

                <div className="space-y-2">
                  <Label htmlFor="branch">Production Branch</Label>
                  <Input
                    id="branch"
                    value={productionBranch}
                    onChange={(e) => setProductionBranch(e.target.value)}
                  />
                </div>

                <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                  <div className="space-y-2">
                    <Label htmlFor="build">Build Command</Label>
                    <Input
                      id="build"
                      value={buildCommand}
                      onChange={(e) => setBuildCommand(e.target.value)}
                      placeholder="npm run build"
                    />
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="output">Output Directory</Label>
                    <Input
                      id="output"
                      value={outputDir}
                      onChange={(e) => setOutputDir(e.target.value)}
                      placeholder="dist"
                    />
                  </div>
                </div>

                <div className="space-y-2">
                  <Label htmlFor="root">Root Directory</Label>
                  <Input
                    id="root"
                    value={rootDir}
                    onChange={(e) => setRootDir(e.target.value)}
                    placeholder="./"
                  />
                  <p className="text-xs text-muted-foreground">
                    Leave empty for repository root
                  </p>
                </div>

                <div className="flex justify-end pt-2">
                  <Button type="submit" disabled={isSaving}>
                    {isSaving ? "Saving..." : "Save Changes"}
                  </Button>
                </div>
              </form>
            </CardContent>
          </Card>

          {/* Environment Variables */}
          <Card>
            <CardHeader>
              <CardTitle>Environment Variables</CardTitle>
              <CardDescription>
                Build vars are passed to Nixpacks; runtime vars are injected when the preview container starts. Import a .env file or add keys manually.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              {/* Import from .env */}
              <div className="space-y-3 rounded-lg border border-border p-4">
                <div className="flex items-center gap-2 text-sm font-medium text-foreground">
                  <FileText className="h-4 w-4" />
                  Import from .env file
                </div>
                <textarea
                  value={dotenvContent}
                  onChange={(e) => setDotenvContent(e.target.value)}
                  placeholder={"DATABASE_URL=postgres://...\nNEXT_PUBLIC_API_URL=https://..."}
                  className="flex min-h-[120px] w-full rounded-md border border-input bg-background px-3 py-2 text-sm font-mono ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                />
                {importError && (
                  <div className="p-3 text-sm text-destructive bg-destructive/10 rounded-md">
                    {importError}
                  </div>
                )}
                <div className="flex flex-col sm:flex-row gap-3 sm:items-center">
                  <Select value={importTarget} onValueChange={(v) => setImportTarget(v as EnvVarTarget)}>
                    <SelectTrigger className="w-32">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="all">All</SelectItem>
                      <SelectItem value="build">Build</SelectItem>
                      <SelectItem value="runtime">Runtime</SelectItem>
                    </SelectContent>
                  </Select>
                  <label className="flex items-center gap-2 text-sm text-muted-foreground">
                    <input
                      type="checkbox"
                      checked={importMerge}
                      onChange={(e) => setImportMerge(e.target.checked)}
                      className="rounded border-input"
                    />
                    Merge with existing
                  </label>
                  <div className="flex gap-2 sm:ml-auto">
                    <Button type="button" variant="outline" asChild>
                      <label className="cursor-pointer">
                        <Upload className="h-4 w-4 mr-2" />
                        Upload file
                        <input
                          type="file"
                          accept=".env,.env.local,.env.production,text/plain"
                          className="hidden"
                          onChange={handleEnvFileUpload}
                        />
                      </label>
                    </Button>
                    <Button
                      type="button"
                      onClick={handleImportEnv}
                      disabled={isImportingEnv || !dotenvContent.trim()}
                    >
                      {isImportingEnv ? "Importing..." : "Import"}
                    </Button>
                  </div>
                </div>
              </div>

              {/* Add new env var */}
              <div className="flex flex-col sm:flex-row gap-3">
                <Input
                  placeholder="KEY"
                  value={newEnvKey}
                  onChange={(e) => setNewEnvKey(e.target.value.toUpperCase())}
                  className="sm:w-40 font-mono"
                />
                <Input
                  placeholder="value"
                  value={newEnvValue}
                  onChange={(e) => setNewEnvValue(e.target.value)}
                  className="flex-1 font-mono"
                  type="password"
                />
                <Select value={newEnvTarget} onValueChange={(v) => setNewEnvTarget(v as EnvVarTarget)}>
                  <SelectTrigger className="w-32">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="all">All</SelectItem>
                    <SelectItem value="build">Build</SelectItem>
                    <SelectItem value="runtime">Runtime</SelectItem>
                  </SelectContent>
                </Select>
                <Button
                  type="button"
                  onClick={handleAddEnvVar}
                  disabled={isAddingEnv || !newEnvKey || !newEnvValue}
                >
                  <Plus className="h-4 w-4" />
                </Button>
              </div>

              {/* Existing env vars */}
              {envLoading ? (
                <div className="text-sm text-muted-foreground">Loading...</div>
              ) : envVars && envVars.length > 0 ? (
                <div className="border rounded-lg divide-y divide-border">
                  {envVars.map((env: EnvVarEntry) => (
                    <div key={env.key} className="flex items-center gap-3 p-3">
                      <span className="font-mono text-sm font-medium w-40 truncate">
                        {env.key}
                      </span>
                      <span className="flex-1 font-mono text-sm text-muted-foreground truncate">
                        {showValues[env.key] ? env.value : "••••••••"}
                      </span>
                      <span className="text-xs text-muted-foreground capitalize w-16">
                        {env.target}
                      </span>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-8 w-8"
                        onClick={() => toggleShowValue(env.key)}
                      >
                        {showValues[env.key] ? (
                          <EyeOff className="h-4 w-4" />
                        ) : (
                          <Eye className="h-4 w-4" />
                        )}
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-8 w-8 text-destructive hover:text-destructive"
                        onClick={() => handleDeleteEnvVar(env.key)}
                      >
                        <Trash2 className="h-4 w-4" />
                      </Button>
                    </div>
                  ))}
                </div>
              ) : (
                <div className="text-sm text-muted-foreground text-center py-4">
                  No environment variables configured
                </div>
              )}
            </CardContent>
          </Card>

          {/* Danger Zone */}
          <Card className="border-destructive/50">
            <CardHeader>
              <CardTitle className="text-destructive">Danger Zone</CardTitle>
              <CardDescription>
                Irreversible and destructive actions
              </CardDescription>
            </CardHeader>
            <CardContent>
              <div className="flex items-center justify-between">
                <div>
                  <p className="font-medium text-foreground">Delete Project</p>
                  <p className="text-sm text-muted-foreground">
                    Permanently delete this project and all its deployments
                  </p>
                </div>
                <Dialog open={showDeleteDialog} onOpenChange={setShowDeleteDialog}>
                  <DialogTrigger asChild>
                    <Button variant="destructive">Delete Project</Button>
                  </DialogTrigger>
                  <DialogContent>
                    <DialogHeader>
                      <DialogTitle className="flex items-center gap-2">
                        <AlertTriangle className="h-5 w-5 text-destructive" />
                        Delete Project
                      </DialogTitle>
                      <DialogDescription>
                        Are you sure you want to delete <strong>{project.name}</strong>?
                        This action cannot be undone. All deployments and data will be permanently deleted.
                      </DialogDescription>
                    </DialogHeader>
                    <DialogFooter>
                      <Button variant="outline" onClick={() => setShowDeleteDialog(false)}>
                        Cancel
                      </Button>
                      <Button
                        variant="destructive"
                        onClick={handleDeleteProject}
                        disabled={isDeleting}
                      >
                        {isDeleting ? "Deleting..." : "Delete Project"}
                      </Button>
                    </DialogFooter>
                  </DialogContent>
                </Dialog>
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    </DashboardLayout>
  )
}
