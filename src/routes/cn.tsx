import { createFileRoute } from "@tanstack/react-router";

import { CnCatalog } from "@/modules/cn";

export const Route = createFileRoute("/cn")({
  component: CnCatalog,
});
