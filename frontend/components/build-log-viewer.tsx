"use client"

import { useCallback, useEffect, useRef, useState } from "react"
import { api, type DeploymentState } from "@/lib/api"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Terminal, Copy, Check } from "lucide-react"

const ACTIVE_STATES = new Set<DeploymentState>(["queued", "building", "uploading"])

interface BuildLogViewerProps {
  deploymentId: string
  state: DeploymentState
  initialBuildLog?: string
  onStreamEnd?: () => void
}

export function BuildLogViewer({
  deploymentId,
  state,
  initialBuildLog,
  onStreamEnd,
}: BuildLogViewerProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const preRef = useRef<HTMLPreElement>(null)
  const fullLogRef = useRef("")
  const pinnedRef = useRef(true)
  const streamStartedRef = useRef(false)

  const [isStreaming, setIsStreaming] = useState(false)
  const [hasLogs, setHasLogs] = useState(Boolean(initialBuildLog?.trim()))
  const [copied, setCopied] = useState(false)

  const scrollIfPinned = useCallback(() => {
    const el = containerRef.current
    if (!el || !pinnedRef.current) return
    el.scrollTop = el.scrollHeight
  }, [])

  const appendLines = useCallback(
    (lines: string[]) => {
      if (lines.length === 0) return

      const chunk = lines.join("\n")
      const prefix = fullLogRef.current.length > 0 ? "\n" : ""
      fullLogRef.current += prefix + chunk

      const pre = preRef.current
      if (pre) {
        pre.appendChild(document.createTextNode(prefix + chunk))
      }

      setHasLogs(true)
      scrollIfPinned()
    },
    [scrollIfPinned],
  )

  const clearLogs = useCallback(() => {
    fullLogRef.current = ""
    if (preRef.current) {
      preRef.current.textContent = ""
    }
    setHasLogs(false)
  }, [])

  const handleScroll = useCallback(() => {
    const el = containerRef.current
    if (!el) return
    pinnedRef.current = el.scrollHeight - el.scrollTop - el.clientHeight < 48
  }, [])

  useEffect(() => {
    if (streamStartedRef.current || !initialBuildLog?.trim()) return
    if (fullLogRef.current.length > 0) return

    fullLogRef.current = initialBuildLog
    if (preRef.current) {
      preRef.current.textContent = initialBuildLog
    }
    setHasLogs(true)
    scrollIfPinned()
  }, [initialBuildLog, scrollIfPinned])

  useEffect(() => {
    if (streamStartedRef.current) return
    if (!ACTIVE_STATES.has(state)) return

    streamStartedRef.current = true
    setIsStreaming(true)

    const stop = api.streamDeploymentLogs(
      deploymentId,
      appendLines,
      () => {
        setIsStreaming(false)
        onStreamEnd?.()
      },
      () => {
        setIsStreaming(false)
      },
      () => {
        clearLogs()
      },
    )

    return stop
  }, [deploymentId, state, appendLines, clearLogs, onStreamEnd])

  const copyLogs = () => {
    navigator.clipboard.writeText(fullLogRef.current)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <Card id="logs">
      <CardHeader className="flex flex-row items-center justify-between">
        <CardTitle className="flex items-center gap-2">
          <Terminal className="h-5 w-5" />
          Build Logs
          {isStreaming && (
            <span className="ml-2 h-2 w-2 rounded-full bg-success animate-pulse" />
          )}
        </CardTitle>
        <Button variant="outline" size="sm" onClick={copyLogs} disabled={!hasLogs}>
          {copied ? (
            <>
              <Check className="h-4 w-4 mr-1" />
              Copied
            </>
          ) : (
            <>
              <Copy className="h-4 w-4 mr-1" />
              Copy
            </>
          )}
        </Button>
      </CardHeader>
      <CardContent className="p-0">
        <div
          ref={containerRef}
          onScroll={handleScroll}
          className="relative bg-black rounded-b-lg max-h-[500px] overflow-y-auto overflow-x-hidden"
        >
          <pre
            ref={preRef}
            className="log-viewer min-h-[120px] p-4 font-mono text-sm text-foreground/90 m-0 whitespace-pre-wrap break-all"
          />
          {!hasLogs && (
            <div className="pointer-events-none absolute inset-0 flex items-center justify-center p-8 text-center text-muted-foreground">
              {state === "queued"
                ? "Waiting for build to start..."
                : "No build logs available"}
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  )
}
