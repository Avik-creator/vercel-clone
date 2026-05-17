"use client"

import { Suspense, useEffect, useState } from "react"
import { useRouter, useSearchParams } from "next/navigation"
import { useAuth } from "@/lib/auth-context"
import { api } from "@/lib/api"

function AuthCallbackContent() {
  const router = useRouter()
  const searchParams = useSearchParams()
  const { setUser } = useAuth()
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    const token = searchParams.get("token")
    
    if (token) {
      api.setToken(token)
      // Fetch user data using the new /me endpoint
      api.getMe()
        .then((user) => {
          setUser(user)
          localStorage.setItem("auth_user", JSON.stringify(user))
          router.push("/dashboard")
        })
        .catch((err) => {
          console.error("Failed to fetch user:", err)
          setError("Failed to complete sign in. Please try again.")
          api.logout()
          setTimeout(() => router.push("/login?error=oauth_failed"), 2000)
        })
    } else {
      router.push("/login?error=oauth_failed")
    }
  }, [searchParams, router, setUser])

  if (error) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-background">
        <div className="text-center">
          <p className="text-destructive">{error}</p>
        </div>
      </div>
    )
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-background">
      <div className="text-center">
        <div className="h-8 w-8 mx-auto animate-spin rounded-full border-2 border-foreground border-t-transparent" />
        <p className="mt-4 text-muted-foreground">Completing sign in...</p>
      </div>
    </div>
  )
}

export default function AuthCallbackPage() {
  return (
    <Suspense fallback={
      <div className="min-h-screen flex items-center justify-center bg-background">
        <div className="text-center">
          <div className="h-8 w-8 mx-auto animate-spin rounded-full border-2 border-foreground border-t-transparent" />
          <p className="mt-4 text-muted-foreground">Loading...</p>
        </div>
      </div>
    }>
      <AuthCallbackContent />
    </Suspense>
  )
}
