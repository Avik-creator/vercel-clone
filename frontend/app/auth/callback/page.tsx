"use client"

import { useEffect } from "react"
import { useRouter, useSearchParams } from "next/navigation"
import { useAuth } from "@/lib/auth-context"
import { api } from "@/lib/api"

export default function AuthCallbackPage() {
  const router = useRouter()
  const searchParams = useSearchParams()
  const { setUser } = useAuth()

  useEffect(() => {
    const token = searchParams.get("token")
    
    if (token) {
      api.setToken(token)
      // In a real app, you'd fetch user data here
      // For now, we'll just redirect to dashboard
      router.push("/dashboard")
    } else {
      router.push("/login?error=oauth_failed")
    }
  }, [searchParams, router, setUser])

  return (
    <div className="min-h-screen flex items-center justify-center bg-background">
      <div className="text-center">
        <div className="h-8 w-8 mx-auto animate-spin rounded-full border-2 border-foreground border-t-transparent" />
        <p className="mt-4 text-muted-foreground">Completing sign in...</p>
      </div>
    </div>
  )
}
