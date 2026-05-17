"use client"

import { useEffect } from "react"
import { useRouter } from "next/navigation"
import Link from "next/link"
import { useAuth } from "@/lib/auth-context"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Key, User, Shield, Bell } from "lucide-react"

const settingsNav = [
  {
    title: "API Keys",
    description: "Manage API keys for programmatic access",
    href: "/settings/api-keys",
    icon: Key,
  },
  {
    title: "Profile",
    description: "Update your profile information",
    href: "/settings/profile",
    icon: User,
  },
  {
    title: "Security",
    description: "Password and authentication settings",
    href: "/settings/security",
    icon: Shield,
  },
  {
    title: "Notifications",
    description: "Configure email notifications",
    href: "/settings/notifications",
    icon: Bell,
  },
]

export default function SettingsPage() {
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
    <div>
      <div className="mb-8">
        <h1 className="text-2xl font-bold text-foreground">Settings</h1>
        <p className="text-muted-foreground mt-1">
          Manage your account settings and preferences
        </p>
      </div>

      {/* User Info */}
      <Card className="mb-8">
        <CardContent className="p-6">
          <div className="flex items-center gap-4">
            <div className="flex h-16 w-16 items-center justify-center rounded-full bg-muted text-2xl font-semibold">
              {user?.name?.charAt(0).toUpperCase() || "U"}
            </div>
            <div>
              <h2 className="text-xl font-semibold text-foreground">{user?.name}</h2>
              <p className="text-muted-foreground">{user?.email}</p>
              {user?.github_login && (
                <p className="text-sm text-muted-foreground mt-1">
                  GitHub: @{user.github_login}
                </p>
              )}
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Settings Navigation */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        {settingsNav.map((item) => (
          <Link key={item.href} href={item.href}>
            <Card className="h-full transition-colors hover:bg-muted/50">
              <CardHeader>
                <div className="flex items-center gap-3">
                  <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-muted">
                    <item.icon className="h-5 w-5 text-foreground" />
                  </div>
                  <div>
                    <CardTitle className="text-base">{item.title}</CardTitle>
                    <CardDescription>{item.description}</CardDescription>
                  </div>
                </div>
              </CardHeader>
            </Card>
          </Link>
        ))}
      </div>
    </div>
  )
}
