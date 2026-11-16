import type { LucideIcon } from "lucide-react";
import { ArrowRight, CircleAlert, Clock } from "lucide-react";
import { Link } from "react-router-dom";
import { Badge, Panel, SectionLabel } from "@/components/ui";

export function PlannedPage({
  title,
  eyebrow,
  description,
  icon: Icon,
  features,
  dependsOn,
}: {
  title: string;
  eyebrow: string;
  description: string;
  icon: LucideIcon;
  features: string[];
  dependsOn?: string;
}) {
  return (
    <div className="space-y-4 rawjan-enter">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <div className="text-[0.63rem] font-semibold uppercase tracking-[0.18em] text-zinc-500">{eyebrow}</div>
          <h2 className="mt-1 font-display text-[1.7rem] leading-tight text-zinc-50">{title}</h2>
          <p className="mt-1 max-w-2xl text-xs leading-relaxed text-zinc-500">{description}</p>
        </div>
        <Badge tone="champagne">
          <Clock className="h-3 w-3" /> Planned
        </Badge>
      </div>

      <Panel className="relative overflow-hidden p-6 sm:p-8">
        <div
          className="pointer-events-none absolute -right-20 -top-24 h-56 w-56 rounded-full opacity-40 blur-3xl"
          style={{ background: "radial-gradient(circle, rgba(179,36,58,0.32), transparent 70%)" }}
        />
        <div className="relative max-w-2xl">
          <div className="flex h-12 w-12 items-center justify-center rounded-xl border border-lily-500/25 bg-lily-600/10 text-lily-300">
            <Icon className="h-5 w-5" strokeWidth={1.7} />
          </div>
          <h3 className="mt-4 font-display text-xl text-zinc-50">This module is not built yet</h3>
          <p className="mt-2 text-sm leading-relaxed text-zinc-500">
            It is intentionally marked as planned rather than shown with sample numbers. Rawjan only displays
            figures that come from your own ledger or from a clearly labelled manual input.
          </p>

          <SectionLabel className="mt-6">What it will cover</SectionLabel>
          <ul className="mt-2 space-y-1.5">
            {features.map((feature) => (
              <li key={feature} className="flex items-start gap-2 text-[0.78rem] leading-relaxed text-zinc-400">
                <span className="mt-[0.42rem] h-1 w-1 shrink-0 rounded-full bg-lily-400/80" />
                {feature}
              </li>
            ))}
          </ul>

          {dependsOn ? (
            <div className="mt-5 flex items-start gap-2 rounded-xl border border-white/[0.06] bg-white/[0.015] px-3.5 py-2.5 text-[0.68rem] leading-relaxed text-zinc-500">
              <CircleAlert className="mt-0.5 h-3.5 w-3.5 shrink-0 text-champagne-500" />
              {dependsOn}
            </div>
          ) : null}

          <div className="mt-6 flex flex-wrap gap-2">
            <Link
              to="/transactions"
              className="inline-flex items-center gap-1.5 rounded-lg border border-white/10 bg-white/[0.035] px-3 py-1.5 text-xs text-zinc-200 transition hover:border-white/20"
            >
              Open the ledger <ArrowRight className="h-3.5 w-3.5" />
            </Link>
            <Link
              to="/settings"
              className="inline-flex items-center gap-1.5 rounded-lg border border-transparent px-3 py-1.5 text-xs text-zinc-500 transition hover:text-zinc-200"
            >
              Review settings
            </Link>
          </div>
        </div>
      </Panel>
    </div>
  );
}
