"use client"

import { useEffect } from "react"
import { useRouter } from "next/navigation"
import Link from "next/link"
import { useAuth } from "@/lib/auth-context"
import { DashboardLayout } from "@/components/dashboard-layout"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { ArrowLeft, Shield, Github } from "lucide-react"

export default function SecurityPage() {
  const { isAuthenticated, isLoading: authLoading, user } = useAuth()
  const router = useRouter()

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

  return (
    <DashboardLayout>
      <div className="mx-auto max-w-4xl px-4 py-8 sm:px-6 lg:px-8">
        <Link
          href="/settings"
          className="inline-flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground mb-6"
        >
          <ArrowLeft className="h-4 w-4" />
          Back to Settings
        </Link>

        <div className="mb-8">
          <h1 className="text-2xl font-bold text-foreground">Security</h1>
          <p className="text-muted-foreground mt-1">
            Manage your password and authentication settings
          </p>
        </div>

        <div className="space-y-6">
          {/* Password Change */}
          <Card>
            <CardHeader>
              <div className="flex items-center gap-3">
                <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-muted">
                  <Shield className="h-5 w-5 text-foreground" />
                </div>
                <div>
                  <CardTitle>Change Password</CardTitle>
                  <CardDescription>Update your password</CardDescription>
                </div>
              </div>
            </CardHeader>
            <CardContent>
              {user?.github_id && !user?.password_hash ? (
                <div className="text-sm text-muted-foreground">
                  You signed up with GitHub. To set a password, use the forgot password feature.
                </div>
              ) : (
                <form className="space-y-4">
                  <div className="space-y-2">
                    <Label htmlFor="current">Current Password</Label>
                    <Input id="current" type="password" />
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="new">New Password</Label>
                    <Input id="new" type="password" />
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="confirm">Confirm New Password</Label>
                    <Input id="confirm" type="password" />
                  </div>
                  <div className="flex justify-end pt-2">
                    <Button type="button" disabled>
                      Update Password
                    </Button>
                  </div>
                </form>
              )}
            </CardContent>
          </Card>

          {/* Connected Accounts */}
          <Card>
            <CardHeader>
              <div className="flex items-center gap-3">
                <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-muted">
                  <Github className="h-5 w-5 text-foreground" />
                </div>
                <div>
                  <CardTitle>Connected Accounts</CardTitle>
                  <CardDescription>Manage your connected accounts</CardDescription>
                </div>
              </div>
            </CardHeader>
            <CardContent>
              <div className="flex items-center justify-between p-4 border border-border rounded-lg">
                <div className="flex items-center gap-3">
                  <Github className="h-5 w-5" />
                  <div>
                    <p className="font-medium text-foreground">GitHub</p>
                    <p className="text-sm text-muted-foreground">
                      {user?.github_login 
                        ? `Connected as @${user.github_login}` 
                        : "Not connected"}
                    </p>
                  </div>
                </div>
                <Button variant="outline" size="sm" disabled>
                  {user?.github_login ? "Disconnect" : "Connect"}
                </Button>
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    </DashboardLayout>
  )
}
