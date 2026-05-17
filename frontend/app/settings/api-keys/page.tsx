"use client"

import { useEffect, useState } from "react"
import { useRouter } from "next/navigation"
import Link from "next/link"
import { useAuth } from "@/lib/auth-context"
import { useApiKeys } from "@/lib/hooks"
import { api, ApiKey } from "@/lib/api"
import { DashboardLayout } from "@/components/dashboard-layout"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Skeleton } from "@/components/ui/skeleton"
import { ArrowLeft, Plus, Trash2, Key, Copy, Check, AlertTriangle } from "lucide-react"
import { formatDate, formatRelativeTime } from "@/lib/utils"
import { mutate } from "swr"

export default function ApiKeysPage() {
  const { isAuthenticated, isLoading: authLoading } = useAuth()
  const router = useRouter()
  const { data: apiKeys, isLoading: keysLoading } = useApiKeys()

  // Create key dialog
  const [showCreateDialog, setShowCreateDialog] = useState(false)
  const [keyName, setKeyName] = useState("")
  const [expiresIn, setExpiresIn] = useState("never")
  const [isCreating, setIsCreating] = useState(false)
  const [createdKey, setCreatedKey] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)

  // Delete dialog
  const [keyToDelete, setKeyToDelete] = useState<ApiKey | null>(null)
  const [isDeleting, setIsDeleting] = useState(false)

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

  const handleCreateKey = async () => {
    setIsCreating(true)
    try {
      const result = await api.createApiKey({
        name: keyName,
        expires_in_days: expiresIn === "never" ? undefined : parseInt(expiresIn),
      })
      setCreatedKey(result.key_plain || null)
      mutate("api-keys")
    } catch (error) {
      console.error("Failed to create key:", error)
    } finally {
      setIsCreating(false)
    }
  }

  const handleCopyKey = () => {
    if (createdKey) {
      navigator.clipboard.writeText(createdKey)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    }
  }

  const handleCloseCreateDialog = () => {
    setShowCreateDialog(false)
    setKeyName("")
    setExpiresIn("never")
    setCreatedKey(null)
    setCopied(false)
  }

  const handleDeleteKey = async () => {
    if (!keyToDelete) return
    setIsDeleting(true)
    try {
      await api.revokeApiKey(keyToDelete.id)
      mutate("api-keys")
      setKeyToDelete(null)
    } catch (error) {
      console.error("Failed to delete key:", error)
    } finally {
      setIsDeleting(false)
    }
  }

  return (
    <DashboardLayout>
      <div className="mx-auto max-w-4xl px-4 py-8 sm:px-6 lg:px-8">
        {/* Back link */}
        <Link
          href="/settings"
          className="inline-flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground mb-6"
        >
          <ArrowLeft className="h-4 w-4" />
          Back to Settings
        </Link>

        <div className="flex items-center justify-between mb-8">
          <div>
            <h1 className="text-2xl font-bold text-foreground">API Keys</h1>
            <p className="text-muted-foreground mt-1">
              Manage API keys for programmatic access to your account
            </p>
          </div>
          <Dialog open={showCreateDialog} onOpenChange={setShowCreateDialog}>
            <DialogTrigger asChild>
              <Button className="gap-2">
                <Plus className="h-4 w-4" />
                Create Key
              </Button>
            </DialogTrigger>
            <DialogContent>
              {!createdKey ? (
                <>
                  <DialogHeader>
                    <DialogTitle>Create API Key</DialogTitle>
                    <DialogDescription>
                      Create a new API key for programmatic access to your account.
                    </DialogDescription>
                  </DialogHeader>
                  <div className="space-y-4 py-4">
                    <div className="space-y-2">
                      <Label htmlFor="name">Key Name</Label>
                      <Input
                        id="name"
                        placeholder="My API Key"
                        value={keyName}
                        onChange={(e) => setKeyName(e.target.value)}
                      />
                    </div>
                    <div className="space-y-2">
                      <Label htmlFor="expires">Expiration</Label>
                      <Select value={expiresIn} onValueChange={setExpiresIn}>
                        <SelectTrigger>
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="never">Never</SelectItem>
                          <SelectItem value="7">7 days</SelectItem>
                          <SelectItem value="30">30 days</SelectItem>
                          <SelectItem value="90">90 days</SelectItem>
                          <SelectItem value="365">1 year</SelectItem>
                        </SelectContent>
                      </Select>
                    </div>
                  </div>
                  <DialogFooter>
                    <Button variant="outline" onClick={handleCloseCreateDialog}>
                      Cancel
                    </Button>
                    <Button onClick={handleCreateKey} disabled={isCreating || !keyName}>
                      {isCreating ? "Creating..." : "Create Key"}
                    </Button>
                  </DialogFooter>
                </>
              ) : (
                <>
                  <DialogHeader>
                    <DialogTitle className="flex items-center gap-2">
                      <Check className="h-5 w-5 text-success" />
                      API Key Created
                    </DialogTitle>
                    <DialogDescription>
                      Copy your API key now. You won&apos;t be able to see it again.
                    </DialogDescription>
                  </DialogHeader>
                  <div className="py-4">
                    <div className="flex items-center gap-2 p-3 bg-muted rounded-lg">
                      <code className="flex-1 text-sm font-mono break-all">
                        {createdKey}
                      </code>
                      <Button variant="ghost" size="icon" onClick={handleCopyKey}>
                        {copied ? (
                          <Check className="h-4 w-4 text-success" />
                        ) : (
                          <Copy className="h-4 w-4" />
                        )}
                      </Button>
                    </div>
                    <p className="mt-3 text-sm text-muted-foreground">
                      Store this key securely. It provides full access to your account.
                    </p>
                  </div>
                  <DialogFooter>
                    <Button onClick={handleCloseCreateDialog}>Done</Button>
                  </DialogFooter>
                </>
              )}
            </DialogContent>
          </Dialog>
        </div>

        {/* API Keys List */}
        <Card>
          <CardHeader>
            <CardTitle>Your API Keys</CardTitle>
            <CardDescription>
              Keys can be used to authenticate API requests
            </CardDescription>
          </CardHeader>
          <CardContent className="p-0">
            {keysLoading ? (
              <div className="p-4 space-y-4">
                {[1, 2, 3].map((i) => (
                  <div key={i} className="flex items-center justify-between">
                    <div>
                      <Skeleton className="h-5 w-32 mb-2" />
                      <Skeleton className="h-4 w-24" />
                    </div>
                    <Skeleton className="h-8 w-8" />
                  </div>
                ))}
              </div>
            ) : apiKeys && apiKeys.length > 0 ? (
              <div className="divide-y divide-border">
                {apiKeys.map((key) => (
                  <div key={key.id} className="flex items-center justify-between p-4">
                    <div>
                      <div className="flex items-center gap-2">
                        <Key className="h-4 w-4 text-muted-foreground" />
                        <span className="font-medium text-foreground">{key.name}</span>
                      </div>
                      <div className="flex items-center gap-4 mt-1 text-sm text-muted-foreground">
                        <span>Created {formatRelativeTime(key.created_at)}</span>
                        {key.last_used_at && (
                          <span>Last used {formatRelativeTime(key.last_used_at)}</span>
                        )}
                        {key.expires_at && (
                          <span>
                            Expires {formatDate(key.expires_at)}
                          </span>
                        )}
                      </div>
                    </div>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="text-destructive hover:text-destructive"
                      onClick={() => setKeyToDelete(key)}
                    >
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  </div>
                ))}
              </div>
            ) : (
              <div className="text-center py-8">
                <Key className="mx-auto h-12 w-12 text-muted-foreground mb-4" />
                <h3 className="font-semibold text-foreground mb-1">No API keys</h3>
                <p className="text-sm text-muted-foreground mb-4">
                  Create an API key to access the API programmatically.
                </p>
                <Button size="sm" onClick={() => setShowCreateDialog(true)}>
                  Create API Key
                </Button>
              </div>
            )}
          </CardContent>
        </Card>

        {/* Delete Confirmation Dialog */}
        <Dialog open={!!keyToDelete} onOpenChange={(open) => !open && setKeyToDelete(null)}>
          <DialogContent>
            <DialogHeader>
              <DialogTitle className="flex items-center gap-2">
                <AlertTriangle className="h-5 w-5 text-destructive" />
                Revoke API Key
              </DialogTitle>
              <DialogDescription>
                Are you sure you want to revoke the API key &quot;{keyToDelete?.name}&quot;?
                This action cannot be undone and any applications using this key will lose access.
              </DialogDescription>
            </DialogHeader>
            <DialogFooter>
              <Button variant="outline" onClick={() => setKeyToDelete(null)}>
                Cancel
              </Button>
              <Button
                variant="destructive"
                onClick={handleDeleteKey}
                disabled={isDeleting}
              >
                {isDeleting ? "Revoking..." : "Revoke Key"}
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      </div>
    </DashboardLayout>
  )
}
