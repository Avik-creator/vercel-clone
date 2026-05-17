import { DeploymentState } from "@/lib/api"
import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"

interface DeploymentStatusBadgeProps {
  state: DeploymentState
  className?: string
}

export function DeploymentStatusBadge({ state, className }: DeploymentStatusBadgeProps) {
  const config: Record<DeploymentState, { variant: "success" | "warning" | "error" | "info" | "secondary"; label: string }> = {
    queued: { variant: "secondary", label: "Queued" },
    building: { variant: "warning", label: "Building" },
    uploading: { variant: "info", label: "Uploading" },
    ready: { variant: "success", label: "Ready" },
    error: { variant: "error", label: "Error" },
    cancelled: { variant: "secondary", label: "Cancelled" },
  }

  const { variant, label } = config[state]

  return (
    <Badge variant={variant} className={cn("gap-1.5", className)}>
      <span className={cn(
        "h-1.5 w-1.5 rounded-full",
        variant === "success" && "bg-success animate-none",
        variant === "warning" && "bg-warning animate-pulse",
        variant === "error" && "bg-error",
        variant === "info" && "bg-info animate-pulse",
        variant === "secondary" && "bg-muted-foreground"
      )} />
      {label}
    </Badge>
  )
}
