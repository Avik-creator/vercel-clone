"use client"

import { useEffect } from "react"
import { useRouter } from "next/navigation"
import { useAuth } from "@/lib/auth-context"
import Link from "next/link"
import { Button } from "@/components/ui/button"
import { Triangle, ArrowRight, Github, Zap, Shield, Globe } from "lucide-react"

export default function HomePage() {
  const { isAuthenticated, isLoading } = useAuth()
  const router = useRouter()

  useEffect(() => {
    if (!isLoading && isAuthenticated) {
      router.push("/dashboard")
    }
  }, [isAuthenticated, isLoading, router])

  if (isLoading) {
    return (
      <div className="flex min-h-screen items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-foreground border-t-transparent" />
      </div>
    )
  }

  return (
    <div className="min-h-screen bg-background">
      {/* Header */}
      <header className="border-b border-border">
        <div className="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8">
          <div className="flex h-16 items-center justify-between">
            <div className="flex items-center gap-2">
              <Triangle className="h-6 w-6 fill-foreground" />
              <span className="text-xl font-semibold">Deploy</span>
            </div>
            <div className="flex items-center gap-4">
              <Link href="/login">
                <Button variant="ghost">Log In</Button>
              </Link>
              <Link href="/register">
                <Button>Sign Up</Button>
              </Link>
            </div>
          </div>
        </div>
      </header>

      {/* Hero Section */}
      <main>
        <section className="relative overflow-hidden py-20 sm:py-32">
          <div className="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8">
            <div className="mx-auto max-w-3xl text-center">
              <h1 className="text-4xl font-bold tracking-tight text-foreground sm:text-6xl text-balance">
                Deploy your projects with zero configuration
              </h1>
              <p className="mt-6 text-lg leading-8 text-muted-foreground text-pretty">
                Push your code to GitHub and watch it deploy automatically. 
                Fast builds, instant rollbacks, and preview deployments for every branch.
              </p>
              <div className="mt-10 flex items-center justify-center gap-4">
                <Link href="/register">
                  <Button size="lg" className="gap-2">
                    Start Deploying
                    <ArrowRight className="h-4 w-4" />
                  </Button>
                </Link>
                <Link href="/login">
                  <Button size="lg" variant="outline" className="gap-2">
                    <Github className="h-4 w-4" />
                    Continue with GitHub
                  </Button>
                </Link>
              </div>
            </div>
          </div>
        </section>

        {/* Features */}
        <section className="border-t border-border py-20">
          <div className="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8">
            <div className="mx-auto max-w-2xl text-center">
              <h2 className="text-3xl font-bold tracking-tight text-foreground">
                Everything you need to ship fast
              </h2>
              <p className="mt-4 text-muted-foreground">
                Focus on your code, we handle the infrastructure.
              </p>
            </div>
            <div className="mx-auto mt-16 max-w-5xl">
              <div className="grid grid-cols-1 gap-8 sm:grid-cols-2 lg:grid-cols-3">
                <div className="rounded-xl border border-border bg-card p-6">
                  <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-muted">
                    <Zap className="h-5 w-5 text-foreground" />
                  </div>
                  <h3 className="mt-4 text-lg font-semibold text-foreground">Instant Deploys</h3>
                  <p className="mt-2 text-sm text-muted-foreground">
                    Push to deploy. Every commit triggers a build and deploys your changes in seconds.
                  </p>
                </div>
                <div className="rounded-xl border border-border bg-card p-6">
                  <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-muted">
                    <Github className="h-5 w-5 text-foreground" />
                  </div>
                  <h3 className="mt-4 text-lg font-semibold text-foreground">GitHub Integration</h3>
                  <p className="mt-2 text-sm text-muted-foreground">
                    Connect your repositories and get automatic deployments for every push and pull request.
                  </p>
                </div>
                <div className="rounded-xl border border-border bg-card p-6">
                  <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-muted">
                    <Globe className="h-5 w-5 text-foreground" />
                  </div>
                  <h3 className="mt-4 text-lg font-semibold text-foreground">Preview Deployments</h3>
                  <p className="mt-2 text-sm text-muted-foreground">
                    Every branch gets its own preview URL. Share and test changes before going live.
                  </p>
                </div>
                <div className="rounded-xl border border-border bg-card p-6">
                  <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-muted">
                    <Shield className="h-5 w-5 text-foreground" />
                  </div>
                  <h3 className="mt-4 text-lg font-semibold text-foreground">Secure by Default</h3>
                  <p className="mt-2 text-sm text-muted-foreground">
                    Encrypted environment variables, automatic SSL, and DDoS protection included.
                  </p>
                </div>
                <div className="rounded-xl border border-border bg-card p-6 sm:col-span-2 lg:col-span-2">
                  <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-muted">
                    <Triangle className="h-5 w-5 fill-foreground" />
                  </div>
                  <h3 className="mt-4 text-lg font-semibold text-foreground">Framework Agnostic</h3>
                  <p className="mt-2 text-sm text-muted-foreground">
                    Deploy any framework - Next.js, React, Vue, Svelte, or static sites. 
                    We detect your framework and configure the build automatically.
                  </p>
                </div>
              </div>
            </div>
          </div>
        </section>
      </main>

      {/* Footer */}
      <footer className="border-t border-border py-8">
        <div className="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2 text-muted-foreground">
              <Triangle className="h-4 w-4 fill-current" />
              <span className="text-sm">Deploy</span>
            </div>
            <p className="text-sm text-muted-foreground">
              Built with Rust and Next.js
            </p>
          </div>
        </div>
      </footer>
    </div>
  )
}
