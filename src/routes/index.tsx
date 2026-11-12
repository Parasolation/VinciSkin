import { createFileRoute, redirect } from "@tanstack/react-router";

/* The manager keeps three pages - the Chinese client's catalog, diagnostics and
   settings - so `/` hands the reader to the catalog rather than drawing a page
   of its own. */
export const Route = createFileRoute("/")({
  beforeLoad: () => {
    throw redirect({ to: "/cn" });
  },
});
