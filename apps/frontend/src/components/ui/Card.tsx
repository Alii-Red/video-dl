import type { HTMLAttributes, ReactNode } from "react";

/** Surface container: rounded, bordered, subtle elevation. Render as a `div`
 * (default) or `li` for list items. */
export function Card({
  as: Tag = "div",
  className = "",
  children,
  ...rest
}: {
  as?: "div" | "li";
  children: ReactNode;
} & HTMLAttributes<HTMLElement>) {
  return (
    <Tag
      className={`rounded-xl border border-neutral-800 bg-neutral-900/60 shadow-sm ${className}`}
      {...rest}
    >
      {children}
    </Tag>
  );
}
