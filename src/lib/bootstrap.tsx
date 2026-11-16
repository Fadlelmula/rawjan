import { createContext, useContext } from "react";
import type { ReactNode } from "react";
import type { Bootstrap } from "@/lib/api";

const BootstrapContext = createContext<Bootstrap | null>(null);

export function BootstrapProvider({
  value,
  children,
}: {
  value: Bootstrap;
  children: ReactNode;
}) {
  return <BootstrapContext.Provider value={value}>{children}</BootstrapContext.Provider>;
}

export function useBootstrap(): Bootstrap {
  const context = useContext(BootstrapContext);
  if (!context) throw new Error("useBootstrap must be used inside BootstrapProvider");
  return context;
}
