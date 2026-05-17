"use client"

import Link from "next/link"
import { usePathname } from "next/navigation"
import { DashboardLayout } from "@/components/dashboard-layout"
import { cn } from "@/lib/utils"
import { Key, User, Shield, Bell } from "lucide-react"

const settingsNav = [
  {
    title: "Profile",
    href: "/settings/profile",
    icon: User,
    description: "Manage your account settings",
  },
  {
    title: "API Keys",
    href: "/settings/api-keys",
    icon: Key,
    description: "Manage API access tokens",
  },
  {
    title: "Security",
    href: "/settings/security",
    icon: Shield,
    description: "Password and security settings",
  },
  {
    title: "Notifications",
    href: "/settings/notifications",
    icon: Bell,
    description: "Configure notifications",
  },
]

export default function SettingsLayout({
  children,
}: {
  children: React.ReactNode
}) {
  const pathname = usePathname()

  return (
    <DashboardLayout>
      <div className="flex flex-col gap-8 lg:flex-row">
        <nav className="lg:w-64 shrink-0">
          <div className="flex flex-col gap-1">
            {settingsNav.map((item) => {
              const isActive = pathname === item.href
              return (
                <Link
                  key={item.href}
                  href={item.href}
                  className={cn(
                    "flex items-center gap-3 rounded-lg px-3 py-2.5 text-sm transition-colors",
                    isActive
                      ? "bg-accent text-accent-foreground"
                      : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
                  )}
                >
                  <item.icon className="h-4 w-4" />
                  {item.title}
                </Link>
              )
            })}
          </div>
        </nav>
        <div className="flex-1 min-w-0">{children}</div>
      </div>
    </DashboardLayout>
  )
}
