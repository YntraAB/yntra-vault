import { cn } from "@/lib/utils"

function Skeleton({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="skeleton"
      className={cn("bg-muted/30 animate-pulse rounded-md select-none", className)}
      {...props}
    />
  )
}

export { Skeleton }



