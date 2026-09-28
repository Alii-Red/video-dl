import type { ButtonHTMLAttributes, ReactNode } from "react";

type Variant = "primary" | "secondary" | "ghost" | "danger";
type Size = "sm" | "md";

const base =
  "inline-flex items-center justify-center gap-2 rounded-lg font-medium transition-colors " +
  "disabled:cursor-not-allowed disabled:opacity-50 focus-visible:outline-none";

const variants: Record<Variant, string> = {
  primary:
    "bg-accent-600 text-white hover:bg-accent-500 active:bg-accent-700 shadow-sm shadow-accent-600/20",
  secondary:
    "border border-neutral-700 bg-neutral-900 text-neutral-200 hover:bg-neutral-800 hover:border-neutral-600",
  ghost: "text-neutral-300 hover:bg-neutral-800 hover:text-neutral-100",
  danger:
    "border border-red-900/70 bg-red-950/40 text-red-300 hover:bg-red-900/40 hover:text-red-200",
};

const sizes: Record<Size, string> = {
  sm: "px-2.5 py-1.5 text-xs",
  md: "px-4 py-2 text-sm",
};

export function Button({
  variant = "primary",
  size = "md",
  className = "",
  children,
  ...rest
}: {
  variant?: Variant;
  size?: Size;
  children: ReactNode;
} & ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      className={`${base} ${variants[variant]} ${sizes[size]} ${className}`}
      {...rest}
    >
      {children}
    </button>
  );
}

/** Square icon-only button (toolbar affordances). Give it an `aria-label`. */
export function IconButton({
  variant = "ghost",
  className = "",
  children,
  ...rest
}: {
  variant?: Variant;
  children: ReactNode;
} & ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      className={`${base} ${variants[variant]} h-8 w-8 rounded-lg p-0 ${className}`}
      {...rest}
    >
      {children}
    </button>
  );
}
