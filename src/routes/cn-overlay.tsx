import { createFileRoute } from "@tanstack/react-router";

import { CnOverlayWindow } from "@/modules/cn/components/CnOverlayWindow";

export const Route = createFileRoute("/cn-overlay")({
  component: CnOverlayWindow,
});
