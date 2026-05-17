"use client"

import { useEffect } from "react"
import { useRouter } from "next/navigation"
import { useAuth } from "@/lib/auth-context"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Bell, Mail, Rocket, AlertTriangle } from "lucide-react"

export default function NotificationsPage() {
  const { isAuthenticated, isLoading: authLoading } = useAuth()
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
    <div>
      <div className="mb-8">
        <h1 className="text-2xl font-bold text-foreground">Notifications</h1>
        <p className="text-muted-foreground mt-1">
          Configure how you receive notifications
        </p>
      </div>

      <Card>
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-muted">
              <Bell className="h-5 w-5 text-foreground" />
            </div>
            <div>
              <CardTitle>Email Notifications</CardTitle>
              <CardDescription>Choose what emails you receive</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between p-4 border border-border rounded-lg">
            <div className="flex items-center gap-3">
              <Rocket className="h-5 w-5 text-muted-foreground" />
              <div>
                <p className="font-medium text-foreground">Deployment Updates</p>
                <p className="text-sm text-muted-foreground">
                  Get notified when deployments succeed or fail
                </p>
              </div>
            </div>
            <input 
              type="checkbox" 
              className="h-4 w-4 rounded border-gray-300"
              defaultChecked 
              disabled
            />
          </div>
          <div className="flex items-center justify-between p-4 border border-border rounded-lg">
            <div className="flex items-center gap-3">
              <AlertTriangle className="h-5 w-5 text-muted-foreground" />
              <div>
                <p className="font-medium text-foreground">Build Failures</p>
                <p className="text-sm text-muted-foreground">
                  Get notified when builds fail
                </p>
              </div>
            </div>
            <input 
              type="checkbox" 
              className="h-4 w-4 rounded border-gray-300"
              defaultChecked 
              disabled
            />
          </div>
          <div className="flex items-center justify-between p-4 border border-border rounded-lg">
            <div className="flex items-center gap-3">
              <Mail className="h-5 w-5 text-muted-foreground" />
              <div>
                <p className="font-medium text-foreground">Weekly Summary</p>
                <p className="text-sm text-muted-foreground">
                  Receive a weekly summary of your deployments
                </p>
              </div>
            </div>
            <input 
              type="checkbox" 
              className="h-4 w-4 rounded border-gray-300"
              disabled
            />
          </div>
          <p className="text-sm text-muted-foreground text-center pt-4">
            Notification settings are coming soon
          </p>
        </CardContent>
      </Card>
    </div>
  )
}
